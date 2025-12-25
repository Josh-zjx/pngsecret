# PngSecret

A steganography tool for embedding secret messages in PNG images using Least Significant Bit (LSB) encoding.

## Overview

PngSecret allows you to hide text messages within PNG images by modifying the least significant bits of the image's RGBA pixel data. The changes are virtually imperceptible to the human eye, making it an effective tool for covert communication.

## Features

- **Encode Mode**: Embed secret text messages into PNG images
- **Decode Mode**: Extract hidden messages from PNG images
- **Extensible Architecture**: Support for custom encoder/decoder implementations
- **Cross-Platform**: Works on Linux, Windows, and macOS

## Installation

### From Source

Ensure you have Rust installed, then:

```bash
git clone https://github.com/Josh-zjx/pngsecret.git
cd pngsecret
cargo build --release
```

The binary will be available at `target/release/pngsecret`.

## Usage

### Encoding a Message

To embed a secret message into an image:

```bash
pngsecret -e -i input.png --text "Your secret message"
```

This will create `input.enc.png` with the hidden message.

You can also specify a custom output filename:

```bash
pngsecret -e -i input.png -o output.png --text "Your secret message"
```

### Decoding a Message

To extract a hidden message from an image:

```bash
pngsecret -i encoded_image.png
```

### Command Line Options

| Option | Short | Description |
|--------|-------|-------------|
| `--input` | `-i` | Input PNG image file (RGBA format expected) |
| `--output` | `-o` | Output filename (optional, defaults to `*.enc.png`) |
| `--encode` | `-e` | Enable encoding mode (decode by default) |
| `--text` | | The secret message to embed (default: "Hello World") |
| `--silent` | `-s` | Reduce stdout output |

### Examples

```bash
# Encode "Hello World" into image.png
pngsecret -e -i image.png

# Encode a custom message
pngsecret -e -i photo.png --text "Top secret information"

# Decode a message (silent mode)
pngsecret -s -i encoded.png

# Encode with custom output path
pngsecret -e -i input.png -o /path/to/output.png --text "Secret"
```

## How It Works

PngSecret uses LSB (Least Significant Bit) steganography:

1. **Encoding**: Each byte of your message is converted into 8 bits. Each bit is then stored in the least significant bit of consecutive bytes in the image's RGBA data.

2. **Message Termination**: A null byte (0x00) is appended to the message to mark its end.

3. **Decoding**: The least significant bits are extracted from the image data, reconstructed into bytes, and the message is read until a null byte is encountered.

### Capacity

The maximum message length depends on the image size:
- Each character requires 8 bits (8 bytes of image data)
- An RGBA pixel has 4 bytes (R, G, B, A channels)
- Maximum characters ≈ (width × height × 4) / 8 = width × height / 2

For example, a 1000×1000 image can store approximately 500,000 characters.

## Architecture

The codebase uses a trait-based architecture for extensibility:

- **`PngSecretEncoder`**: Trait for implementing custom message encoders
- **`PngSecretDecoder`**: Trait for implementing custom message decoders
- **`NaiveEncoder`**: Default encoder that simply null-terminates the message
- **`NaiveDecoder`**: Default decoder that returns extracted bytes as-is

This design allows for future extensions such as:
- Message encryption
- Compression
- Error correction codes

## Testing

Run the test suite:

```bash
cargo test
```

The test suite includes:
- Unit tests for `byte_to_8bits` conversion
- Unit tests for `NaiveEncoder` and `NaiveDecoder`
- Unit tests for output filename generation
- Integration tests for encode/decode roundtrips
- Property-based tests using QuickCheck

## Building

### Debug Build

```bash
cargo build
```

### Release Build

```bash
cargo build --release
```

Release builds are optimized with:
- LTO (Link Time Optimization)
- Single codegen unit
- Stripped symbols
- Panic abort

## License

See the [LICENSE](LICENSE) file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
