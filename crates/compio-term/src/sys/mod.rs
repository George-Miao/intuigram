cfg_select! {
    unix => {
        mod unix;
        pub(crate) use unix::EventStream;
    }
    _ => {
        mod unsupported;
        pub(crate) use unsupported::EventStream;
    }
}
