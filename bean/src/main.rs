use beanrust::*;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let e = core::types::Entry {
        date: "foo".to_string(),
    };
    log::info!("Hello, world! {}", e.date);
}
