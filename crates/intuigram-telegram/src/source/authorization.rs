pub(super) fn normalize_authorization(
    authorization: tl::enums::auth::Authorization,
) -> Result<AuthorizedUser> {
    let authorization = match authorization {
        tl::enums::auth::Authorization::Authorization(authorization) => authorization,
        tl::enums::auth::Authorization::SignUpRequired(_) => return SignUpRequiredSnafu.fail(),
    };
    match authorization.user {
        tl::enums::User::User(user) => Ok(AuthorizedUser {
            id: user.id,
            display_name: user_display_name(&user),
            username: user.username,
        }),
        tl::enums::User::Empty(_) => EmptyAuthorizedUserSnafu.fail(),
    }
}

type PasswordParameters<'a> = (&'a Vec<u8>, &'a Vec<u8>, &'a Vec<u8>, &'a i32);

pub(super) fn password_parameters(
    algorithm: &tl::enums::PasswordKdfAlgo,
) -> Result<PasswordParameters<'_>> {
    match algorithm {
        tl::enums::PasswordKdfAlgo::Sha256Sha256Pbkdf2Hmacsha512iter100000Sha256ModPow(
            algorithm,
        ) => Ok((
            &algorithm.salt1,
            &algorithm.salt2,
            &algorithm.p,
            &algorithm.g,
        )),
        tl::enums::PasswordKdfAlgo::Unknown => UnsupportedPasswordAlgorithmSnafu.fail(),
    }
}
use super::*;
