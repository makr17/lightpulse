use std::env;
use std::f32;
use std::i64;
use std::process::exit;
use std::thread::sleep;

extern crate getopts;
use getopts::Options;
extern crate houselights;
use houselights::houselights::{RGB,Zone,Dmx,kelvin,scale_rgb,render};
extern crate rand;
use rand::distributions::{IndependentSample,Range};
extern crate time;
use time::Duration;

struct Params {
    decay:         f32,
    max_intensity: f32,
    runfor:        i64,
    sleep:         std::time::Duration,
    ranges:        Vec<RGBRange>,
    threshold:     f32
}

#[derive(Clone,Debug)]
struct Pixel { age: u32, rgb: RGB }

struct RGBRange {
    low:    RGB,
    high:   RGB,
    range:  Range<f32>,
}

impl RGBRange {
    fn new() -> RGBRange {
        return RGBRange {
            low:   RGB { red: 0, green: 0, blue: 0 },
            high:  RGB { red: 0, green: 0, blue: 0 },
            range: Range::new(0_f32, 1_f32)
        }
    }
    fn pick(&self) -> RGB {
        let choice: RGB = RGB {
            red:   self._u8_in_range(self.low.red,   self.high.red),
            green: self._u8_in_range(self.low.green, self.high.green),
            blue:  self._u8_in_range(self.low.blue,  self.high.blue),
        };
        return choice;
    }
    fn _u8_in_range (&self, a: u8, b: u8) -> u8 {
        let mut bounds: Vec<u8> = vec![];
        bounds.push(a);
        bounds.push(b);
        bounds.sort();
        //println!("{:?}", bounds);
        let mut rng = rand::thread_rng();
        let choice: u8 =
            (bounds[0] as f32
             + self.range.ind_sample(&mut rng)
             * (bounds[1] - bounds[0]) as f32
            ).round() as u8;
        return choice;
    }
}

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} [options]", program);
    print!("{}", opts.usage(&brief));
}

fn parse_temp (input: String) -> RGBRange {
    let tokens: Vec<u16> = input.split(":").map(|x| x.parse::<u16>().unwrap()).collect();
    let mut range = RGBRange::new();
    range.low  = kelvin(tokens[0]);
    range.high = kelvin(tokens[1]);
    return range;
}

fn parse_color(input: String) -> RGBRange {
    //println!("parse_color({})", input);
    let colors: Vec<String> = input.split(":").map(|x| x.parse::<String>().unwrap()).collect();
    let mut range = RGBRange::new();
    range.low  = hex2rgb(&colors[0]);
    range.high = hex2rgb(&colors[1]);
    return range;
}

fn hex2rgb(hex: &String) -> RGB {
    return RGB {
        red:   match i32::from_str_radix(&hex[0..2], 16) {
            Ok(m)   => { m as u8 },
            Err(_f) => { 0 }  // TODO: error?
        },
        green: match i32::from_str_radix(&hex[2..4], 16) {
            Ok(m)   => { m as u8 },
            Err(_f) => { 0 }  // TODO: error?
        },
        blue:  match i32::from_str_radix(&hex[4..6], 16) {
            Ok(m)   => { m as u8 },
            Err(_f) => { 0 }  // TODO: error?
        }
    }
}

fn age2intensity(age: u32) -> f32 {
    // stretch x by a factor of 4, to stay lit longer
    let fage = (age as f32)/4.0;
    // log-normal probability density function for the long tail we want
    // break out sigma and mu to make it easier to tweak moving forward
    let sigma = 0.5;
    let mu    = 0.0;
    let intensity = 1.0/(sigma * (2.0 * std::f32::consts::PI).sqrt())
        * (-1.0 * ((fage.ln() - mu) * (fage.ln() - mu))/(2.0 * sigma * sigma)).exp();
    // scale by max_intensity
    return intensity;
}

fn build_params () -> Params {
    // seed default params
    let mut params = Params {
        decay: 0.002,
        max_intensity: 0.8,
        runfor: std::i32::MAX as i64,
        sleep: Duration::nanoseconds(20_000_000).to_std().unwrap(),
        ranges: vec![],
        threshold: 0.001
    };

    // parse command line args and adjust params accordingly
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();
    let mut opts = Options::new();
    opts.optopt("d", "decay", "slow decay by this factor, defaults to 2", "DECAY");
    opts.optmulti(
        "e",
        "temprange",
        "light temp range, in kelvin, default 2700:5500",
        "LOW:HIGH"
    );
    opts.optmulti(
        "g",
        "rgbrange",
        "color range, RGB in Hex, format RRGGBB:RRGGBB, no default",
        "LOW:HIGH"
    );
    opts.optflag("h", "help", "print this help menu");
    opts.optopt("m", "maxintensity", "maximum brightness, 1..255, default 75", "MAX");
    opts.optopt("r", "runfor", "number of minutes to run, default 5", "MINUTES");
    opts.optopt("s", "sleep", "sleep interval in seconds, default 1.5", "SECONDS");
    opts.optopt(
        "t",
        "threshold",
        "probablity that a pixel lights up, default 0.10",
        "THRESHOLD"
    );
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => { m }
        Err(f) => { panic!(f.to_string()) }
    };
    if matches.opt_present("h") {
        print_usage(&program, opts);
        exit(0);
    }
    if matches.opt_present("d") {
        params.decay = matches.opt_str("d").unwrap().parse::<f32>().unwrap();
    }
    if matches.opt_present("e") {
        for t in matches.opt_strs("e") {
            params.ranges.push(parse_temp(t));
        }
    }
    if matches.opt_present("g") {
        for c in matches.opt_strs("g") {
            params.ranges.push(parse_color(c));
        }
    }
    if matches.opt_present("m") {
        let max: u8 = matches.opt_str("m").unwrap().parse::<u8>().unwrap();
        params.max_intensity = (max as f32)/255_f32
    }
    if matches.opt_present("r") {
        params.runfor = matches.opt_str("r").unwrap().parse::<i64>().unwrap();
    }
    if matches.opt_present("s") {
        // take float seconds
        // convert to int seconds and nanoseconds to make Duration happy
        let seconds: f32 = matches.opt_str("s").unwrap().parse::<f32>().unwrap();
        let whole_seconds: i64 = seconds as i64;
        let nano_seconds: i64 = ((seconds - whole_seconds as f32) * 1_000_000_000_f32) as i64;
        params.sleep = (Duration::seconds(whole_seconds) + Duration::nanoseconds(nano_seconds)).to_std().unwrap();
    }
    if matches.opt_present("t") {
        params.threshold = matches.opt_str("t").unwrap().parse::<f32>().unwrap();
    }
    if params.ranges.len() == 0 {
        // default to one range, warm white to cool white
        params.ranges.push(RGBRange { low: kelvin(2700), high: kelvin(5500), range: Range::new(0_f32, 1_f32) });
    }
    return params;
}

fn main() {
    let params = build_params();

    let dmx = Dmx::new();
    
    let zones: [Zone; 6] = [
        Zone { head: 0, body: 44, tail: 3, name: "10".to_string() },
        Zone { head: 2, body: 91, tail: 3, name: "11a".to_string() },
        Zone { head: 2, body: 92, tail: 2, name: "11b".to_string() },
        Zone { head: 2, body: 90, tail: 3, name: "12a".to_string() },
        Zone { head: 2, body: 91, tail: 3, name: "12b".to_string() },
        Zone { head: 2, body: 43, tail: 0, name: "13".to_string() }
    ];

    let mut lights: Vec<Pixel> = vec![];
    // TODO: probably a more idiomatic way to built the default state
    for zone in zones.iter() {
        for _i in 0..zone.body {
            let pixel = Pixel {
                age: 0,
                rgb: RGB { red: 0, green: 0, blue: 0 },
            };
            lights.push(pixel);
        }
    }

    let mut rng = rand::thread_rng();
    let zero_to_one = Range::new(0_f32, 1_f32);
    let color_picker = Range::new(0, params.ranges.len());
    
    let finish = time::get_time() + Duration::minutes(params.runfor);
    loop {
        let mut rgb: Vec<RGB> = vec![];
        for light in lights.iter_mut() {
            if light.age == 0 {
                // unlit
                if zero_to_one.ind_sample(&mut rng) < params.threshold {
                    light.age = 1;
                    // pick a random color picker
                    let idx: usize = color_picker.ind_sample(&mut rng) as usize;
                    // and let it choose the color
                    light.rgb = params.ranges[idx].pick();
                }
            }
            if light.age > 0 {
                let intensity = age2intensity(light.age);
                if intensity == 0.0 {
                    light.age = 0;
                    rgb.push(RGB::null())
                }
                else {
                    let value: RGB = scale_rgb(&light.rgb, intensity, params.max_intensity);
                    println!("{:?} * {} = {:?}", light.rgb, intensity, value);
                    rgb.push(value);
                    light.age += 1;
                }
            } else {
                rgb.push(RGB::null())
            }
        }

        // and send it as a slice to render()
        render(&rgb, &zones, &dmx);
        if time::get_time() > finish {
            break;
        }
        sleep(params.sleep);
    }
}
