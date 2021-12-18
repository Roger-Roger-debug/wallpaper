use std::{convert::TryFrom, path::PathBuf, time::Duration};

pub enum MessageArgs {
    Wallpaper,
    Action,
    Duration
}

impl TryFrom<&str> for MessageArgs {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        use MessageArgs::*;
        match value.to_lowercase().as_str() {
            "wallpaper" | "wp" => Ok(Wallpaper),
            "action" | "ac" => Ok(Action),
            "duration" | "dur" => Ok(Duration),
            _ => Err("Not recognized"),
        }
    }
}

pub enum Args {
    Help,
    Stop,
    Next,
    Prev,
    RNG,
    Linear,
    Hold(Option<PathBuf>),
    Update,
    Save,
    Shuffle,
    Interval(Duration),
    Get(MessageArgs),
}

impl TryFrom<&str> for Args {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        use Args::*;
        let value = value.to_lowercase();
        let splits: Vec<&str> = value.split(' ').collect();
        let tupl = (splits.get(0).unwrap_or(&"").to_owned(), splits.get(1));
        match tupl.0 {
            "stop" => Ok(Stop),
            "next" => Ok(Next),
            "prev" => Ok(Prev),
            "rng" => Ok(RNG),
            "linear" | "lin" => Ok(Linear),
            "hold" => {
                let string: Option<PathBuf> = match tupl.1 {
                    Some(val) => Some(val.into()),
                    None => None,
                };
                Ok(Hold(string))
            },
            "update" => Ok(Update),
            "save" => Ok(Save),
            "shuffle" | "shl" => Ok(Shuffle),
            "interval" | "int" => {
                let d = Duration::new(tupl.1.unwrap_or(&"").parse::<u64>().unwrap_or(60), 0);
                Ok(Interval(d))
            }
            "get" => {
                if let Some(arg) = tupl.1 {
                    match *arg {
                        "ac" | "action" => Ok(Get(MessageArgs::Action)),
                        "dur" | "duration" => Ok(Get(MessageArgs::Duration)),
                        "wp" | "wallpaper" => Ok(Get(MessageArgs::Wallpaper)),
                        _ => Err("Not recognized"),
                    }
                } else {
                    Err("Not recognized")
                }
            },
            "help" => Ok(Help),
            _ => Err("Not recognized"),
        }
    }
}
