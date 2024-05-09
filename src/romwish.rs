// Protocol:
// cmd_
// OPTION: 04
// RGB: 05
// RGB_BRIGHT: 07 05
// SCHEDULE: 09


// white strobe speed chg
// 7E04 023D FFFF FF00 EF
// HH11 2233 4444 TTTT TT
// 1: chg
// 2: speed
// 3: Val: 00->64
// 4: TRUE

// white strobe bright chg
// 7E04 04F0 0001 FF00 EF
// HH11 2233 4444 TTTT TT
// 1: chg
// 2: brightness
// 3: 00->64
// 4: TRUE

// stop
// 7E04 0400 0000 FF00 EF
// HH11 2233 4444 TTTT TT
// 1: chg
// 2: brightness
// 3: OFF
// 4: FALSE

// start
// 7E04 04F0 0001 FF00 EF
// HH11 2233 4444 33?? TT
// 1: chg
// 2: brightness
// 3: ON
// 4: TRUE

// manual color: 07 0503

use btleplug::platform::{Adapter, PeripheralId};
use uuid::Uuid;
use btleplug::api::{Central, CentralEvent, Characteristic, Peripheral, ScanFilter, WriteType};
use std::time::Duration;
use rand::{Rng, thread_rng};
use tokio::time;
use tokio_stream::StreamExt;
use itertools::Itertools;

pub async fn find_primary_uuid_for(adapter: &Adapter, prefix: &str) -> crate::Result<Option<Uuid>> {
    println!("Starting scan on {}...", adapter.adapter_info().await?);
    adapter
        .start_scan(ScanFilter::default())
        .await
        .expect("Can't scan BLE adapter for connected devices...");
    let mut evt_stream = adapter.events().await.unwrap();
    let led_id = loop {
        if let Some(ref evt) = evt_stream.next().await
        {
            println!("📲{evt:?}");
            let pid = match evt {
                CentralEvent::DeviceDiscovered(id) => { None }
                CentralEvent::DeviceUpdated(id) => { None }
                CentralEvent::DeviceConnected(id) => { None }
                CentralEvent::DeviceDisconnected(id) => { None }
                CentralEvent::ManufacturerDataAdvertisement { id, manufacturer_data } => { None }
                CentralEvent::ServiceDataAdvertisement { id, service_data } => { Some(id) }
                CentralEvent::ServicesAdvertisement { id, services } => { Some(id) }
            };

            if let Some(id) = pid {
                let name = match adapter.peripheral(id).await {
                    Ok(peripheral) => {
                        peripheral.properties().await?.map(|p| p.local_name).flatten()
                    }
                    Err(e) => None
                };

                println!("📡 {name:?}");

                if name.map(|x| x.starts_with(prefix)).unwrap_or_default() {
                    println!("👌Found it!");
                    break id.clone();
                }
            }
        }
    };

    let leds = adapter.peripheral(&led_id).await?;
    if !leds.is_connected().await? {
        println!("connecting");
        leds.connect().await?;
    }

    leds.discover_services().await?;
    let mut primary = None;
    for service in leds.services() {
        println!(
            "Service UUID {}, primary: {}",
            service.uuid, service.primary
        );
        if service.primary {
            primary = Some(service.uuid)
        }
        for characteristic in service.characteristics {
            println!("  {:?}", characteristic);
        }
    }
    adapter.stop_scan().await?;
    leds.disconnect().await?;
    while leds.is_connected().await? {
        tokio::time::sleep(Duration::from_secs(1)).await;
        println!(".");
    }
    println!("Disconnected");
    Ok(primary)
}

pub async fn do_demo(light: &impl Peripheral, w: &Characteristic) -> crate::Result<()> {
    let mut rng = thread_rng();
    send_command(light, &w, &LightsCommands::Power(true)).await?;

    for _ in 0..100 {
        send_command(light, &w,
                     &LightsCommands::rgby_f32(
                         rng.gen(),
                         rng.gen(),
                         rng.gen(),
                         rng.gen(),
                     )).await?;
        time::sleep(Duration::from_millis(200)).await;
    }
    send_command(light, &w, &LightsCommands::PresetSpeed(1f32)).await?;
    send_command(light, &w, &LightsCommands::Preset(LightPreset::BlueFade)).await?;
    time::sleep(Duration::from_millis(5000)).await;
    send_command(light, &w, &LightsCommands::PresetSpeed(0.1)).await?;
    time::sleep(Duration::from_millis(5000)).await;


    send_command(light, &w, &LightsCommands::Power(false)).await?;
    time::sleep(Duration::from_millis(1000)).await;
    Ok(())
}

pub async fn send_command(light: &impl Peripheral, characteristic: &Characteristic, cmd: &LightsCommands) -> crate::Result<()> {
    println!("➡️ Sending {cmd:?}");
    let bytes = cmd.to_bytes();
    light.write(&characteristic, &bytes, WriteType::WithoutResponse).await?;


    let bstr = bytes.chunks(2).map(hex::encode_upper).join(" ");
    println!("\t{bstr}");
    Ok(())
}

#[derive(Copy, Clone, Debug)]
pub enum LightPreset {
    StaticWhite = 0x86,
    RgbJumping = 0x87,
    RainbowJumping = 0x88,
    RgbFade = 0x89,
    RainbowFade = 0x8A,
    RedFade = 0x8B,
    GreenFade = 0x8C,
    BlueFade = 0x8D,
    YellowFade = 0x8E,
    CyanFade = 0x8F,
    PurpleFade = 0x90,
    WhiteFade = 91,
    RedGreenFade = 0x92,
    RedBlueFade = 0x93,
    GreenBlueFade = 0x94,
    SevenColorsStrobe = 0x95,
    RedStrobe = 0x96,
    GreenStrobe = 0x97,
    BlueStrobe = 0x98,
    YellowStrobe = 0x99,
    CyanStrobe = 0x9A,
    PurpleStrobe = 0x9B,
    WhiteStrobe = 0x9C,
}

fn extract_device_id(e: &CentralEvent) -> &PeripheralId {
    match e {
        CentralEvent::DeviceDiscovered(id) => id,
        CentralEvent::DeviceUpdated(id) => id,
        CentralEvent::DeviceConnected(id) => id,
        CentralEvent::DeviceDisconnected(id) => id,
        CentralEvent::ManufacturerDataAdvertisement { id, .. } => id,
        CentralEvent::ServiceDataAdvertisement { id, .. } => id,
        CentralEvent::ServicesAdvertisement { id, .. } => id,
    }
}

pub async fn wait_for_advertisement(adapter: &Adapter, target: &PeripheralId) -> crate::Result<impl Peripheral> {
    adapter.start_scan(Default::default()).await?;
    let mut evts = adapter.events().await?;
    let found = loop {
        if let Some(CentralEvent::DeviceDiscovered(id)) = evts.next().await {
            if target == &id {
                break adapter.peripheral(&id).await?;
            }
        }
    };
    adapter.stop_scan().await?;
    Ok(found)
}

#[derive(Copy, Clone, Debug)]
pub enum LightsCommands {
    Power(bool),
    Rgb { r: u8, g: u8, b: u8 },
    RgbBrightness { r: u8, g: u8, b: u8, l: u8 },
    PresetBrightness(f32),
    PresetSpeed(f32),
    Preset(LightPreset),
}

impl LightsCommands {
    pub fn rgby_f32(r: f32, g: f32, b: f32, y: f32) -> Self {
        Self::RgbBrightness {
            r: (r * 255f32) as u8,
            g: (g * 255f32) as u8,
            b: (b * 255f32) as u8,
            l: (y * 255f32) as u8,
        }
    }
}

impl LightsCommands {
    pub fn to_bytes(&self) -> [u8; 9] {
        let mut buf = [0x7E, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0x00, 0xEF];

        match self {
            LightsCommands::Power(on) => {
                buf[1] = 0x04;
                buf[2] = 0x04;
                if *on {
                    buf[3] = 0xF0;
                    buf[4] = 0x01
                }
            }
            LightsCommands::Rgb { r, g, b } => {
                buf[1] = 0x07;
                buf[2] = 0x05;
                buf[3] = 0x03;
                buf[4] = *r;
                buf[5] = *g;
                buf[6] = *b;
            }
            LightsCommands::RgbBrightness { r, g, b, l } => {
                buf[1] = 0x07;
                buf[2] = 0x05;
                buf[3] = 0x03;
                buf[4] = *r;
                buf[5] = *g;
                buf[6] = *b;
                buf[7] = *l;
            }
            LightsCommands::PresetBrightness(bright) => {
                buf[1] = 0x04;
                buf[2] = 0x01;
                buf[3] = 0x03;
                buf[4] = (*bright * (0x64 as f32)) as u8;
                buf[5] = 0xff;
                buf[6] = 0xff;
                buf[7] = 0xff;
            }
            LightsCommands::PresetSpeed(spd) => {}
            LightsCommands::Preset(preset) => {
                buf[1] = 05;
                buf[2] = 03;
                buf[3] = *preset as u8;
                buf[4] = 0x03; //?
                buf[5] = 0xff; //?
            }
        }
        buf
    }
}
