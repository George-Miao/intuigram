impl Client {
    /// Requests delivery of a Telegram login code.
    pub async fn request_login_code(&mut self, phone_number: String) -> Result<CodeRequest> {
        let mut restarts = 0;
        loop {
            let response = self
                .connection
                .invoke(&tl::functions::auth::SendCode {
                    phone_number: phone_number.clone(),
                    api_id: self.credentials.api_id,
                    api_hash: self.credentials.api_hash.clone(),
                    settings: tl::types::CodeSettings {
                        allow_flashcall: false,
                        current_number: false,
                        allow_app_hash: false,
                        allow_missed_call: false,
                        allow_firebase: false,
                        unknown_number: false,
                        logout_tokens: None,
                        token: None,
                        app_sandbox: None,
                    }
                    .into(),
                })
                .await;
            match response {
                Ok(response) => return self.normalize_code_request(phone_number, response),
                Err(source) => {
                    if let Some(dc_id) = rpc_migration_dc(&source, "PHONE_MIGRATE_") {
                        return PhoneMigrationSnafu { dc_id }.fail();
                    }
                    if login_error_action(&source) == LoginErrorAction::Restart
                        && restarts < MAX_LOGIN_RESTARTS
                    {
                        restarts += 1;
                        compio::time::sleep(Duration::from_millis(250)).await;
                        continue;
                    }
                    return Err(Error::Invoke { source });
                }
            }
        }
    }

    /// Requests Telegram's next available delivery method for a login code.
    pub async fn resend_login_code(&mut self, token: &LoginCodeToken) -> Result<CodeRequest> {
        let response = self
            .connection
            .invoke(&tl::functions::auth::ResendCode {
                phone_number: token.phone_number.clone(),
                phone_code_hash: token.phone_code_hash.clone(),
                reason: None,
            })
            .await
            .context(InvokeSnafu)?;
        self.normalize_code_request(token.phone_number.clone(), response)
    }

    /// Submits the delivered login code.
    pub async fn sign_in_with_code(
        &mut self,
        token: &LoginCodeToken,
        code: String,
    ) -> Result<CodeSignIn> {
        let response = self
            .connection
            .invoke(&tl::functions::auth::SignIn {
                phone_number: token.phone_number.clone(),
                phone_code_hash: token.phone_code_hash.clone(),
                phone_code: Some(code),
                email_verification: None,
            })
            .await;
        match response {
            Ok(authorization) => normalize_authorization(authorization).map(|identity| {
                self.identity = Some(identity.clone());
                CodeSignIn::Authorized(identity)
            }),
            Err(error) if error.is_rpc("SESSION_PASSWORD_NEEDED") => self
                .begin_password_challenge()
                .await
                .map(CodeSignIn::PasswordRequired),
            Err(source) => Err(Error::Invoke { source }),
        }
    }

    pub(super) async fn begin_password_challenge(&mut self) -> Result<PasswordPrompt> {
        let password: tl::types::account::Password = self
            .connection
            .invoke(&tl::functions::account::GetPassword {})
            .await
            .context(InvokeSnafu)?
            .into();
        let prompt = PasswordPrompt {
            hint: password.hint.clone(),
        };
        self.password = Some(password);
        Ok(prompt)
    }

    fn normalize_code_request(
        &mut self,
        phone_number: String,
        response: tl::enums::auth::SentCode,
    ) -> Result<CodeRequest> {
        match response {
            tl::enums::auth::SentCode::Code(code) => Ok(CodeRequest::Sent(LoginCodeToken {
                phone_number,
                phone_code_hash: code.phone_code_hash,
                delivery: normalize_code_delivery(code.r#type),
                next_delivery: code.next_type.as_ref().map(normalize_code_delivery_method),
                next_delivery_after: code.timeout.filter(|timeout| *timeout >= 0),
            })),
            tl::enums::auth::SentCode::Success(success) => {
                normalize_authorization(success.authorization).map(|identity| {
                    self.identity = Some(identity.clone());
                    CodeRequest::AlreadyAuthorized(identity)
                })
            }
            tl::enums::auth::SentCode::PaymentRequired(_) => LoginPaymentRequiredSnafu.fail(),
        }
    }

    /// Completes Telegram SRP two-factor authentication.
    pub async fn sign_in_with_password(&mut self, password: &[u8]) -> Result<AuthorizedUser> {
        let info = self
            .password
            .clone()
            .context(MissingPasswordChallengeSnafu)?;
        let algorithm = info
            .current_algo
            .as_ref()
            .context(IncompletePasswordParametersSnafu)?;
        let (salt1, salt2, prime, generator) = password_parameters(algorithm)?;
        if !check_p_and_g(prime, generator) {
            return UnsupportedPasswordAlgorithmSnafu.fail();
        }
        let server_b = info
            .srp_b
            .as_ref()
            .context(IncompletePasswordParametersSnafu)?;
        let srp_id = info.srp_id.context(IncompletePasswordParametersSnafu)?;
        let (proof, client_a) = calculate_2fa(
            salt1,
            salt2,
            prime,
            generator,
            server_b.clone(),
            info.secure_random,
            password,
        );
        let authorization = self
            .connection
            .invoke(&tl::functions::auth::CheckPassword {
                password: tl::types::InputCheckPasswordSrp {
                    srp_id,
                    a: client_a.to_vec(),
                    m1: proof.to_vec(),
                }
                .into(),
            })
            .await;
        match authorization {
            Ok(authorization) => normalize_authorization(authorization).inspect(|identity| {
                self.password = None;
                self.identity = Some(identity.clone());
            }),
            Err(source) => {
                self.begin_password_challenge().await?;
                Err(Error::Invoke { source })
            }
        }
    }
}
use super::*;
