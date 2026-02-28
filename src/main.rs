//! # PngSecret
//!
//! A steganography tool for embedding secret messages in PNG images.
//!
//! This tool uses Least Significant Bit (LSB) steganography to hide data
//! within the RGBA channels of PNG images. The changes are virtually
//! imperceptible to the human eye.
//!
//! ## Features
//!
//! - Encode secret text messages into PNG images
//! - Decode hidden messages from PNG images
//! - Supports custom encoder/decoder implementations via traits
//!
//! ## Usage
//!
//! ```bash
//! # Encode a message into an image
//! pngsecret -e -i input.png --text "Secret message"
//!
//! # Decode a message from an image
//! pngsecret -i encoded.png
//! ```
//!
//! ## How it works
//!
//! The tool modifies the least significant bit of each byte in the image's
//! RGBA data to encode the secret message. Since each pixel has 4 channels
//! (R, G, B, A), and each byte of the message requires 8 bits, approximately
//! 2 pixels are needed per character of the message.

use image::RgbaImage;
use std::path::PathBuf;
use std::sync::OnceLock;
use structopt::StructOpt;

/// Error type returned when reading a hidden message fails.
///
/// This error occurs when no valid null-terminated message is found
/// in the image, indicating either no message was embedded or the
/// image data has been corrupted.
#[derive(Debug, Clone)]
struct ReaderError;

/// Global flag to control verbose output.
/// When set to `true`, the application suppresses informational messages.
static SILENT: OnceLock<bool> = OnceLock::new();

/// Command-line options for the PngSecret application.
///
/// This struct defines all available command-line arguments and their
/// default values. It uses `structopt` for parsing.
#[derive(Debug, StructOpt)]
#[structopt(
    name = "PngSecret",
    about = "A simple tool to embed secret bytes to png images"
)]
struct Opt {
    /// Reduce stdout output. When set, only the decoded message is printed.
    #[structopt(short, long, help = "reduce stdout print")]
    silent: bool,

    /// Enable encoding mode. If not set, the tool operates in decode mode.
    #[structopt(short, long, help = "default to decode if not set")]
    encode: bool,

    /// The secret message to embed in the image.
    #[structopt(
        long,
        default_value = "Hello World",
        help = "the secret you want to embed"
    )]
    text: String,

    /// Path to the input PNG image file.
    #[structopt(short, long, parse(from_os_str), help = "RGBA image file expected")]
    input: PathBuf,

    /// Optional path for the output file.
    /// If not specified, the output will be named `<input>.enc.png`.
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

/// Determines the output filename for encoded images.
///
/// If an output path is specified in the options, that path is used.
/// Otherwise, generates a filename by replacing the input file's extension
/// with `.enc.png`.
///
/// # Arguments
///
/// * `opt` - Command-line options containing input/output paths
///
/// # Returns
///
/// The path where the encoded image should be saved
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

/// Converts a single byte into an array of 8 individual bits.
///
/// Each bit is represented as a `u8` value (0 or 1) to simplify
/// arithmetic operations when modifying pixel values.
///
/// # Arguments
///
/// * `byte` - The byte to convert
///
/// # Returns
///
/// An array of 8 `u8` values, each being 0 or 1, representing the
/// bits from most significant to least significant.
///
/// # Example
///
/// ```
/// let bits = byte_to_8bits(&65); // 'A' in ASCII
/// assert_eq!(bits, [0, 1, 0, 0, 0, 0, 0, 1]);
/// ```
fn byte_to_8bits(byte: &u8) -> [u8; 8] {
    let mut x = *byte;
    let mut bits: [u8; 8] = [0; 8];
    for i in 0..8 {
        bits[7 - i] = x % 2;
        x /= 2;
    }
    bits
}

/// Writer for embedding secret messages into PNG images.
///
/// Uses LSB (Least Significant Bit) steganography to encode data
/// into the RGBA channels of the image. Each bit of the message
/// is stored in the least significant bit of consecutive pixel bytes.
///
/// # Fields
///
/// * `buffer` - The RGBA image buffer to write to
/// * `encoder` - The encoder used to prepare the message for embedding
struct PngSecretWriter {
    buffer: RgbaImage,
    encoder: Box<dyn PngSecretEncoder>,
}

impl PngSecretWriter {
    /// Creates a new `PngSecretWriter` with the given image and encoder.
    ///
    /// Prints image dimensions and maximum message capacity when not in silent mode.
    ///
    /// # Arguments
    ///
    /// * `img` - The RGBA image to embed the message into
    /// * `encoder` - The encoder to use for preparing the message
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

    /// Embeds the encoded message into the image and saves it to a file.
    ///
    /// Modifies the least significant bit of each byte in the image buffer
    /// to store the message bits. Prints a warning if the message is too
    /// long for the image.
    ///
    /// # Arguments
    ///
    /// * `output_filename` - Path where the modified image should be saved
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

/// Reader for extracting hidden messages from PNG images.
///
/// Reads the least significant bits from the RGBA channels of an image
/// and reconstructs the original message.
///
/// # Fields
///
/// * `buffer` - The RGBA image buffer to read from
/// * `decoder` - The decoder used to process the extracted message
struct PngSecretReader {
    buffer: RgbaImage,
    decoder: Box<dyn PngSecretDecoder>,
}

impl PngSecretReader {
    /// Creates a new `PngSecretReader` with the given image and decoder.
    ///
    /// Prints image dimensions when not in silent mode.
    ///
    /// # Arguments
    ///
    /// * `img` - The RGBA image to extract the message from
    /// * `decoder` - The decoder to use for processing the message
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

    /// Extracts the hidden message from the image.
    ///
    /// Reads the least significant bits from consecutive bytes in the image,
    /// reconstructs them into bytes, and returns the decoded message.
    /// The message is expected to be null-terminated.
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<u8>)` - The extracted message as a byte vector
    /// * `Err(ReaderError)` - If no null terminator is found (no valid message)
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

/// Trait for encoding messages before embedding into images.
///
/// Implementations of this trait prepare the message data for steganographic
/// embedding. This allows for different encoding schemes such as compression
/// or encryption.
trait PngSecretEncoder {
    /// Encodes the given byte sequence for embedding.
    ///
    /// The encoded data should be stored internally and retrieved via `get_text()`.
    ///
    /// # Arguments
    ///
    /// * `seq` - The raw message bytes to encode
    fn encode(&mut self, seq: &[u8]);

    /// Returns the encoded message ready for embedding.
    ///
    /// # Returns
    ///
    /// The encoded message as a byte vector
    fn get_text(&self) -> Vec<u8>;
}

/// Trait for decoding messages after extraction from images.
///
/// Implementations of this trait process the extracted data to recover
/// the original message. This pairs with `PngSecretEncoder` to support
/// various encoding schemes.
trait PngSecretDecoder {
    /// Decodes the extracted byte sequence.
    ///
    /// # Arguments
    ///
    /// * `seq` - The raw bytes extracted from the image
    ///
    /// # Returns
    ///
    /// The decoded message as a byte vector
    fn decode(&mut self, seq: Vec<u8>) -> Vec<u8>;
}

/// A simple encoder that adds null-termination without transformation.
///
/// This encoder simply appends a null byte (0x00) to the message,
/// which is used as a terminator when reading. No compression or
/// encryption is applied.
struct NaiveEncoder {
    text: Vec<u8>,
}

/// A simple decoder that returns the message without transformation.
///
/// This decoder returns the extracted bytes as-is, without any
/// decompression or decryption.
struct NaiveDecoder {}

impl PngSecretDecoder for NaiveDecoder {
    fn decode(&mut self, seq: Vec<u8>) -> Vec<u8> {
        seq
    }
}

impl NaiveDecoder {
    /// Creates a new `NaiveDecoder`.
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
    /// Creates a new `NaiveEncoder` with an empty message buffer.
    fn new() -> Self {
        NaiveEncoder { text: Vec::new() }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck::quickcheck;

    // ==================== byte_to_8bits tests ====================

    #[test]
    fn byte_to_8bits_zero() {
        let bits = byte_to_8bits(&0u8);
        assert_eq!(bits, [0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn byte_to_8bits_one() {
        let bits = byte_to_8bits(&1u8);
        assert_eq!(bits, [0, 0, 0, 0, 0, 0, 0, 1]);
    }

    #[test]
    fn byte_to_8bits_max() {
        let bits = byte_to_8bits(&255u8);
        assert_eq!(bits, [1, 1, 1, 1, 1, 1, 1, 1]);
    }

    #[test]
    fn byte_to_8bits_mid_value() {
        // 128 = 10000000 in binary
        let bits = byte_to_8bits(&128u8);
        assert_eq!(bits, [1, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn byte_to_8bits_alternating() {
        // 170 = 10101010 in binary
        let bits = byte_to_8bits(&170u8);
        assert_eq!(bits, [1, 0, 1, 0, 1, 0, 1, 0]);
    }

    #[test]
    fn byte_to_8bits_letter_a() {
        // 'A' = 65 = 01000001 in binary
        let bits = byte_to_8bits(&65u8);
        assert_eq!(bits, [0, 1, 0, 0, 0, 0, 0, 1]);
    }

    // ==================== NaiveEncoder tests ====================

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
        let raw_message = "Under the surface, the assert_eq! and assert_ne! macros use the operators == and !=, respectively. When the assertions fail, these macros print their arguments using debug formatting, which means the values being compared must implement the PartialEq and Debug traits.";
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
    fn naive_encoder_appends_null_terminator() {
        let raw_message = "test";
        let mut encoder = NaiveEncoder::new();
        encoder.encode(raw_message.as_bytes());
        let encoded = encoder.get_text();
        assert_eq!(encoded.len(), raw_message.len() + 1);
        assert_eq!(*encoded.last().unwrap(), 0u8);
    }

    #[test]
    fn naive_encoder_new_creates_empty_text() {
        let encoder = NaiveEncoder::new();
        assert!(encoder.get_text().is_empty());
    }

    // ==================== NaiveDecoder tests ====================

    #[test]
    fn naive_decoder_returns_same_input() {
        let mut decoder = NaiveDecoder::new();
        let input = vec![72, 101, 108, 108, 111]; // "Hello"
        let output = decoder.decode(input.clone());
        assert_eq!(input, output);
    }

    #[test]
    fn naive_decoder_handles_empty_input() {
        let mut decoder = NaiveDecoder::new();
        let input: Vec<u8> = vec![];
        let output = decoder.decode(input.clone());
        assert_eq!(input, output);
    }

    #[test]
    fn naive_decoder_handles_binary_data() {
        let mut decoder = NaiveDecoder::new();
        let input = vec![0, 1, 2, 255, 128, 64];
        let output = decoder.decode(input.clone());
        assert_eq!(input, output);
    }

    // ==================== get_output_filename tests ====================

    #[test]
    fn get_output_filename_with_provided_output() {
        let opt = Opt {
            silent: false,
            encode: true,
            text: "test".to_string(),
            input: PathBuf::from("input.png"),
            output: Some(PathBuf::from("custom_output.png")),
        };
        let result = get_output_filename(&opt);
        assert_eq!(result, PathBuf::from("custom_output.png"));
    }

    #[test]
    fn get_output_filename_without_provided_output() {
        let opt = Opt {
            silent: false,
            encode: true,
            text: "test".to_string(),
            input: PathBuf::from("input.png"),
            output: None,
        };
        let result = get_output_filename(&opt);
        assert_eq!(result, PathBuf::from("input.enc.png"));
    }

    #[test]
    fn get_output_filename_with_different_extension() {
        let opt = Opt {
            silent: false,
            encode: true,
            text: "test".to_string(),
            input: PathBuf::from("image.jpeg"),
            output: None,
        };
        let result = get_output_filename(&opt);
        assert_eq!(result, PathBuf::from("image.enc.png"));
    }

    #[test]
    fn get_output_filename_with_path() {
        let opt = Opt {
            silent: false,
            encode: true,
            text: "test".to_string(),
            input: PathBuf::from("/path/to/image.png"),
            output: None,
        };
        let result = get_output_filename(&opt);
        assert_eq!(result, PathBuf::from("/path/to/image.enc.png"));
    }

    // ==================== Integration tests for PngSecretWriter and PngSecretReader ====================

    #[test]
    fn writer_reader_roundtrip_simple() {
        // Create a small 4x4 RGBA image (64 bytes, enough for a short message)
        let img = RgbaImage::from_fn(4, 4, |_, _| image::Rgba([128u8, 128, 128, 255]));
        let message = "Hi";

        // Encode the message
        let mut encoder = NaiveEncoder::new();
        encoder.encode(message.as_bytes());
        let mut buffer = img.clone();
        let text = encoder.get_text();
        let mut text_iter = text.iter().flat_map(byte_to_8bits);
        for pixel in buffer.iter_mut() {
            if let Some(t) = text_iter.next() {
                *pixel = *pixel - (*pixel % 2) + t;
            } else {
                break;
            }
        }

        // Decode the message
        let mut reader = PngSecretReader::new(buffer, Box::new(NaiveDecoder::new()));
        let result = reader.read_image();
        assert!(result.is_ok());
        let decoded_message = String::from_utf8(result.unwrap()).unwrap();
        assert_eq!(decoded_message, message);
    }

    #[test]
    fn writer_reader_roundtrip_empty() {
        let img = RgbaImage::from_fn(4, 4, |_, _| image::Rgba([128u8, 128, 128, 255]));
        let message = "";

        let mut encoder = NaiveEncoder::new();
        encoder.encode(message.as_bytes());
        let mut buffer = img.clone();
        let text = encoder.get_text();
        let mut text_iter = text.iter().flat_map(byte_to_8bits);
        for pixel in buffer.iter_mut() {
            if let Some(t) = text_iter.next() {
                *pixel = *pixel - (*pixel % 2) + t;
            } else {
                break;
            }
        }

        let mut reader = PngSecretReader::new(buffer, Box::new(NaiveDecoder::new()));
        let result = reader.read_image();
        assert!(result.is_ok());
        let decoded_message = String::from_utf8(result.unwrap()).unwrap();
        assert_eq!(decoded_message, message);
    }

    #[test]
    fn writer_reader_roundtrip_longer_message() {
        // Create a larger image to accommodate a longer message
        let img = RgbaImage::from_fn(16, 16, |_, _| image::Rgba([200u8, 100, 50, 255]));
        let message = "The quick brown fox";

        let mut encoder = NaiveEncoder::new();
        encoder.encode(message.as_bytes());
        let mut buffer = img.clone();
        let text = encoder.get_text();
        let mut text_iter = text.iter().flat_map(byte_to_8bits);
        for pixel in buffer.iter_mut() {
            if let Some(t) = text_iter.next() {
                *pixel = *pixel - (*pixel % 2) + t;
            } else {
                break;
            }
        }

        let mut reader = PngSecretReader::new(buffer, Box::new(NaiveDecoder::new()));
        let result = reader.read_image();
        assert!(result.is_ok());
        let decoded_message = String::from_utf8(result.unwrap()).unwrap();
        assert_eq!(decoded_message, message);
    }

    #[test]
    fn reader_fails_without_null_terminator() {
        // Create an image with all 1s in LSB (no null terminator)
        let img = RgbaImage::from_fn(4, 4, |_, _| image::Rgba([255u8, 255, 255, 255]));

        let mut reader = PngSecretReader::new(img, Box::new(NaiveDecoder::new()));
        let result = reader.read_image();
        assert!(result.is_err());
    }

    // ==================== QuickCheck property-based tests ====================

    quickcheck! {
        fn naive_encoder_length(message:String)->bool {
            let raw_message = message;
            let mut encoder = NaiveEncoder::new();
            encoder.encode(raw_message.as_bytes());
            let mut expected_message = Vec::from(raw_message.as_bytes());
            expected_message.push(0);
            let encode_message = encoder.get_text();
            encode_message.len() == expected_message.len()
        }

        fn naive_encoder_content(message: String)->bool {
            let raw_message = message;
            let mut encoder = NaiveEncoder::new();
            encoder.encode(raw_message.as_bytes());
            let mut expected_message = Vec::from(raw_message.as_bytes());
            expected_message.push(0);
            let encode_message = encoder.get_text();
            expected_message.iter().zip(encode_message.iter()).all(|(a, b)| a == b)
        }

        fn byte_to_8bits_roundtrip(byte: u8) -> bool {
            let bits = byte_to_8bits(&byte);
            let reconstructed = bits.iter().fold(0u8, |acc, &bit| acc * 2 + bit);
            reconstructed == byte
        }

        fn naive_decoder_identity(data: Vec<u8>) -> bool {
            let mut decoder = NaiveDecoder::new();
            let result = decoder.decode(data.clone());
            result == data
        }
    }
}
