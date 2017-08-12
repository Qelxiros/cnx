use std::error;
use std::f64;
use std::fs::File;
use std::io::Read;
use std::result;
use std::str::FromStr;
use std::time::Duration;

use tokio_timer::Timer;

use Cnx;
use errors::*;
use text::{Attributes, Color, Text};


#[derive(Clone, Debug, Eq, PartialEq)]
enum Status {
    Full,
    Charging,
    Discharging,
    Unknown,
}

impl FromStr for Status {
    type Err = Error;

    fn from_str(s: &str) -> result::Result<Self, Self::Err> {
        match s {
            "Full" => Ok(Status::Full),
            "Charging" => Ok(Status::Charging),
            "Discharging" => Ok(Status::Discharging),
            "Unknown" => Ok(Status::Unknown),
            _ => bail!("Unknown Status: {}", s),
        }
    }
}


pub struct Battery {
    timer: Timer,
    update_interval: Duration,
    battery: String,
    attr: Attributes,
    warning_color: Color,
}

impl Battery {
    pub fn new(hue: &Cnx, attr: Attributes, warning_color: Color) -> Battery {
        Battery {
            timer: hue.timer(),
            update_interval: Duration::from_secs(60),
            battery: "BAT0".to_owned(),
            attr,
            warning_color
        }
    }

    fn load_value_inner<T>(&self, file: &str) -> Result<T>
    where
        T: FromStr,
        <T as FromStr>::Err: error::Error + Send + 'static,
    {
        let path = format!("/sys/class/power_supply/{}/{}", self.battery, file);
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        FromStr::from_str(contents.trim()).chain_err(|| "Failed to parse value")
    }

    fn load_value<T>(&self, file: &str) -> Result<T>
    where
        T: FromStr,
        <T as FromStr>::Err: error::Error + Send + 'static,
    {
        self.load_value_inner(file).chain_err(|| {
            format!("Could not load value from battery status file: {}", file)
        })
    }

    fn tick(&self) -> Result<Vec<Text>> {
        let full: f64 = self.load_value("energy_full")?;
        let now: f64 = self.load_value("energy_now")?;
        let percentage = (now / full) * 100.0;

        // If we're discharging, show time to empty.
        // If we're charging, show time to full.
        let power: f64 = self.load_value("power_now")?;
        let status: Status = self.load_value("status")?;
        let time = match status {
            Status::Discharging => now / power,
            Status::Charging => (full - now) / power,
            _ => 0.0,
        };
        let hours = time as u64;
        let minutes = (time * 60.0) as u64 % 60;

        let text = format!(
            "({percentage:.0}% - {hours}:{minutes:02})",
            percentage = percentage,
            hours = hours,
            minutes = minutes
        );

        // If we're discharging and have <=10% left, then render with a
        // special warning color.
        let mut attr = self.attr.clone();
        if status == Status::Discharging && percentage <= 10.0 {
            attr.fg_color = self.warning_color.clone()
        }

        Ok(vec![
            Text {
                attr,
                text,
                stretch: false,
            },
        ])
    }
}

timer_widget!(Battery, timer, update_interval, tick);
