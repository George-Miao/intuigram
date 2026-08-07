cfg_select! {
    windows => {
        mod windows;
        pub(crate) use windows::EventStream;
    }
    unix => {
        mod unix;
        pub(crate) use unix::EventStream;
    }
    _ => {
        mod unsupported;
        pub(crate) use unsupported::EventStream;
    }
}
