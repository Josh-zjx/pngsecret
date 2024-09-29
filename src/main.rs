use image::RgbaImage;
use std::path::PathBuf;
use std::sync::OnceLock;
use structopt::StructOpt;

// TODO: Should find better name
#[derive(Debug, Clone)]
struct ReaderError;

static SILENT: OnceLock<bool> = OnceLock::new();

#[derive(Debug, StructOpt)]
#[structopt(
    name = "PngSecret",
    about = "A simple tool to embed secret bytes to png images"
)]
struct Opt {
    #[structopt(short, long, help = "reduce stdout print")]
    silent: bool,

    #[structopt(short, long, help = "default to decode if not set")]
    encode: bool,

    #[structopt(
        long,
        default_value = "Hello World",
        help = "the secret you want to embed"
    )]
    text: String,

    #[structopt(short, long, parse(from_os_str), help = "RGBA image file expected")]
    input: PathBuf,

    #[structopt(
        short,
        long,
        parse(from_os_str),
        help = "optional, output would be *.enc.png if skipped"
    )]
    output: Option<PathBuf>,
}

fn main() {
    let opt = Opt::from_args();
    if opt.silent && SILENT.set(opt.silent).is_err() {
        println!("cannot set global variable silent!");
        return;
    }

    if let Ok(img) = image::open(&opt.input) {
        if opt.encode {
            let output_filename = get_output_filename(&opt);
            if SILENT.get().is_none() {
                println!("output filename {:?}", output_filename);
            }
            let mut writer = PngSecretWriter::new(img.into_rgba8(), Box::new(NaiveEncoder::new()));
            writer.encoder.encode(opt.text.as_bytes());
            writer.write_image(output_filename);
        } else {
            let mut reader = PngSecretReader::new(img.into_rgba8(), Box::new(NaiveDecoder::new()));
            if let Ok(raw_message) = reader.read_image() {
                if let Ok(message) = String::from_utf8(raw_message.clone()) {
                    if SILENT.get().is_none() {
                        println!("Here is the message:");
                    }
                    println!("{:}", message);
                } else {
                    println!("The message cannot printed as string!");
                    // TODO: Implement binary dump here for unparsable message
                }
            } else {
                println!("This image doesn't have embedded message!")
            }
        }
    } else {
        println!("The file {:?} couldn't be correctly read", opt.input);
    }
    #[cfg(debug_assertions)]
    println!("{:?}", opt);
}

fn get_output_filename(opt: &Opt) -> PathBuf {
    match &opt.output {
        Some(path) => path.clone(),
        None => {
            let mut temp = opt.input.to_owned();
            temp.set_extension("enc.png");
            temp
        }
    }
}

/// This function split one byte into 8 bit, the element is still u8 to simplify the addition to
/// pixel
fn byte_to_8bits(byte: &u8) -> [u8; 8] {
    let mut x = *byte;
    let mut bits: [u8; 8] = [0; 8];
    for i in 0..8 {
        bits[7 - i] = x % 2;
        x /= 2;
    }
    bits
}

/// A Writer using the last ONE bit of the pixel RGBA channel to encode the message
struct PngSecretWriter {
    buffer: RgbaImage,
    encoder: Box<dyn PngSecretEncoder>,
}

impl PngSecretWriter {
    fn new(img: RgbaImage, encoder: Box<dyn PngSecretEncoder>) -> Self {
        if SILENT.get().is_none() {
            println!(
                "Image width {:}, Image Height {:}, message length limit {:} bytes",
                img.width(),
                img.height(),
                img.width() * img.height() / 2 - 1,
            );
        }
        PngSecretWriter {
            buffer: img,
            encoder,
        }
    }
    fn write_image(&mut self, output_filename: PathBuf) {
        let text = self.encoder.get_text();
        if (self.buffer.width() * self.buffer.height()) < text.len() as u32 {
            // TODO: Should find more elegant way to handle this error
            println!("You are writing more message than the image could support!");
        }
        let mut text_iter = text.iter().flat_map(byte_to_8bits);
        for i in self.buffer.iter_mut() {
            if let Some(t) = text_iter.next() {
                *i = *i - (*i % 2) + t;
            } else {
                break;
            }
        }
        if self.buffer.save(output_filename.clone()).is_ok() {
            if SILENT.get().is_none() {
                println!("Writing modified image to file {:?}", output_filename);
            }
        } else {
            println!("saving file failure");
        }
    }
}

struct PngSecretReader {
    buffer: RgbaImage,
    decoder: Box<dyn PngSecretDecoder>,
}

impl PngSecretReader {
    fn new(img: RgbaImage, decoder: Box<dyn PngSecretDecoder>) -> Self {
        if SILENT.get().is_none() {
            println!(
                "Image width {:}, Image Height {:}",
                img.width(),
                img.height()
            );
        }
        PngSecretReader {
            buffer: img,
            decoder,
        }
    }
    fn read_image(&mut self) -> Result<Vec<u8>, ReaderError> {
        let mut message: Vec<u8> = Vec::new();
        let pixel_iter = self.buffer.iter();
        let mut count = 0;
        let mut sum = 0;
        for i in pixel_iter {
            sum = sum * 2 + i % 2;
            count += 1;
            if count == 8 {
                if sum == 0 {
                    return Ok(self.decoder.decode(message));
                }
                message.push(sum);
                count = 0;
                sum = 0;
            }
        }
        Err(ReaderError)
    }
}

/// Encoder should support encode and write
/// Could extend to support different encoding format and encryption scheme
trait PngSecretEncoder {
    /// The text should be carried within the encoder
    fn encode(&mut self, seq: &[u8]);
    fn get_text(&self) -> Vec<u8>;
}

/// Decoder
trait PngSecretDecoder {
    fn decode(&mut self, seq: Vec<u8>) -> Vec<u8>;
}

// WARN: Is the data member really needed?
struct NaiveEncoder {
    text: Vec<u8>,
}

struct NaiveDecoder {}

impl PngSecretDecoder for NaiveDecoder {
    fn decode(&mut self, seq: Vec<u8>) -> Vec<u8> {
        seq
    }
}

impl NaiveDecoder {
    fn new() -> Self {
        NaiveDecoder {}
    }
}
impl PngSecretEncoder for NaiveEncoder {
    fn encode(&mut self, seq: &[u8]) {
        self.text = seq.to_vec();
        self.text.push(0);
    }
    fn get_text(&self) -> Vec<u8> {
        self.text.clone()
    }
}

impl NaiveEncoder {
    fn new() -> Self {
        NaiveEncoder { text: Vec::new() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck::quickcheck;

    #[test]
    fn naive_encoder_correct_normal() {
        let raw_message = "Hello World!";
        let mut encoder = NaiveEncoder::new();
        encoder.encode(raw_message.as_bytes());
        let mut expected_message = Vec::from(raw_message.as_bytes());
        let encode_message = encoder.get_text();
        expected_message.push(0);
        println!("{:?} {:?}", expected_message, encode_message);
        assert!(expected_message
            .iter()
            .zip(encode_message.iter())
            .all(|(a, b)| a == b));
    }

    #[test]
    fn naive_encoder_correct_empty() {
        let raw_message = "";
        let mut encoder = NaiveEncoder::new();
        encoder.encode(raw_message.as_bytes());
        let mut expected_message = Vec::from(raw_message.as_bytes());
        let encode_message = encoder.get_text();
        expected_message.push(0);
        println!("{:?} {:?}", expected_message, encode_message);
        assert!(expected_message
            .iter()
            .zip(encode_message.iter())
            .all(|(a, b)| a == b));
    }

    #[test]
    fn naive_encoder_correct_long() {
        let raw_message = "Under the surface, the assert_eq! and assert_ne! macros use the operators == and !=, respectively. When the assertions fail, these macros print their arguments using debug formatting, which means the values being compared must implement the PartialEq and Debug traits. All primitive types and most of the standard library types implement these traits. For structs and enums that you define yourself, you’ll need to implement PartialEq to assert equality of those types. You’ll also need to implement Debug to print the values when the assertion fails. Because both traits are derivable traits, as mentioned in Listing 5-12 in Chapter 5, this is usually as straightforward as adding the #[derive(PartialEq, Debug)] annotation to your struct or enum definition. See Appendix C, “Derivable Traits,” for more details about these and other derivable traits.";
        let mut encoder = NaiveEncoder::new();
        encoder.encode(raw_message.as_bytes());
        let mut expected_message = Vec::from(raw_message.as_bytes());
        let encode_message = encoder.get_text();
        expected_message.push(0);
        println!("{:?} {:?}", expected_message, encode_message);
        assert!(expected_message
            .iter()
            .zip(encode_message.iter())
            .all(|(a, b)| a == b));
    }
    quickcheck! {
        fn naive_encoder_length(message:String)->bool {
            let raw_message = message;
            let mut encoder = NaiveEncoder::new();
            encoder.encode(raw_message.as_bytes());
            let mut expected_message = Vec::from(raw_message.as_bytes());
            expected_message.push(0);
            let encode_message = encoder.get_text();
            return encode_message.len() == expected_message.len() ;

        }

        fn naive_encoder_content(message: String)->bool {
            let raw_message = message;
            let mut encoder = NaiveEncoder::new();
            encoder.encode(raw_message.as_bytes());
            let mut expected_message = Vec::from(raw_message.as_bytes());
            expected_message.push(0);
            let encode_message = encoder.get_text();
            return expected_message.iter().zip(encode_message.iter()).all(|(a, b)| a== b) ;
        }

    }
}
