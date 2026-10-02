// SPDX-License-Identifier: GPL-3.0-only

use clap_lex::RawArgs;
use std::error::Error;

mod app;
mod config;
mod graph;
mod icons;
mod info;
mod localize;
mod theme;
mod widget;

use app::App;

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let raw_args = RawArgs::from_args();
    let mut cursor = raw_args.cursor();
    while let Some(arg) = raw_args.next_os(&mut cursor) {
        match arg.to_str() {
            Some("--help") | Some("-h") => {
                print_help();
                return Ok(());
            }
            Some("--version") | Some("-V") => {
                println!("iced_systemmonitor {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            _ => {}
        }
    }

    localize::localize();

    iced::application(App::new, App::update, App::view)
        .title(App::title)
        .theme(App::theme)
        .subscription(App::subscription)
        .settings(iced::Settings {
            id: Some(config::APP_ID.to_string()),
            ..Default::default()
        })
        .window(iced::window::Settings {
            size: iced::Size::new(1280.0, 800.0),
            min_size: Some(iced::Size::new(360.0, 180.0)),
            ..Default::default()
        })
        .antialiasing(true)
        .run()?;

    Ok(())
}

fn print_help() {
    println!(
        r#"Iced System Monitor
A system monitor built with iced.

Options:
  --help                          Show this message
  --version                       Show the version of iced_systemmonitor

License: GPL-3.0-only"#
    );
}
