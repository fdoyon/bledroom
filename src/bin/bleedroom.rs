use itertools::Itertools;
use std::io::Write;
use std::time::Duration;
use btleplug::api::{BDAddr, Central, CharPropFlags, Manager as _, Peripheral};
use btleplug::platform::{Manager, PeripheralId};
use tokio_stream::StreamExt;
use uuid::uuid;
use clap::{Parser, Subcommand};
use ble_bled::LightsCommands;


enum AppCommand {
    TurnOn,
    Wait,
    TurnOff,
    Pink,
    Demo,
}

#[derive(Subcommand, Copy, Clone, Debug)]
enum ClapCommand {
    On,
    Off,
    Pink,
}

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    commands: ClapCommand,
}

#[tokio::main]
async fn main() -> ble_bled::Result<()> {
    pretty_env_logger::init();
    let args = Args::parse();
    let cmds =
        match args.commands {
            ClapCommand::On => vec![AppCommand::TurnOn, AppCommand::Wait],
            ClapCommand::Off => vec![AppCommand::TurnOff, AppCommand::Wait],
            ClapCommand::Pink => vec![AppCommand::TurnOn, AppCommand::Wait, AppCommand::Pink, AppCommand::Wait],
        };
    //vec![AppCommand::TurnOn, AppCommand::Wait(Duration::from_millis(100)), AppCommand::Pink, AppCommand::Wait(Duration::from_millis(100))];
    let manager = Manager::new().await?;

    let adapter_list = manager.adapters().await?;
    if adapter_list.is_empty() {
        eprintln!("No Bluetooth adapters found");
    }

    for adapter in adapter_list.iter() {
        // TODO: Discover for real, using services and capacities, but the LED strip doesn't seem to respond to broadcasts...
        let target = uuid!("2d2ad3df-b026-8803-df5e-3f53530d1259").into();
        println!("Looking for {target:X?}");

        let light = ble_bled::wait_for_advertisement(&adapter, &target).await?;
        println!("Found {light:X?}");

        // connect to the device
        light.connect().await?;

        // discover services and characteristics
        light.discover_services().await?;

        // find the characteristic we want
        let chars = light.characteristics();
        let mut writable = Vec::new();
        for c in chars {
            println!("{c:#?}");
            if c.properties.contains(CharPropFlags::READ) {
                print!("🔎 Reading:");
                std::io::stdout().flush()?;
                let status = light.read(&c).await?;
                let str_h = hex::encode(status);
                println!("{str_h}");
            }

            if c.properties.contains(CharPropFlags::WRITE_WITHOUT_RESPONSE) {
                writable.push(c.clone())
            }

            if c.properties.contains(CharPropFlags::NOTIFY) {
                println!("Sub to notifications");
                let ll = light.clone();
                tokio::spawn(async move {
                    let mut notifications = ll.notifications().await.unwrap();
                    while let Some(notif) = notifications.next().await {
                        println!("==>{notif:X?}")
                    }
                });
            }
        }

        for w in writable {
            println!("Writing to {w:#?}");

            for cmd in &cmds {
                match cmd {
                    AppCommand::TurnOn => ble_bled::send_command(&light, &w, &LightsCommands::Power(true)).await?,
                    AppCommand::TurnOff => ble_bled::send_command(&light, &w, &LightsCommands::Power(false)).await?,
                    AppCommand::Pink => ble_bled::send_command(&light, &w, &LightsCommands::rgby_f32(01f32, 0f32, 0.2f32, 0.4f32)).await?,
                    AppCommand::Demo => ble_bled::do_demo(&light, &w).await?,
                    AppCommand::Wait => tokio::time::sleep(Duration::from_millis(100)).await,
                }
            }
        }
    }
    Ok(())
}
