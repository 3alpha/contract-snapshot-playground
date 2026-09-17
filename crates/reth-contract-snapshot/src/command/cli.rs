use super::error::BlockNumberParseError;
use alloy_primitives::Address;
use clap::Parser;
use reth_contract_snapshot::DEFAULT_PAGE_BYTES;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "reth-contract-snapshot",
    version,
    about = "Export one or more accounts' complete persistent state from a live Reth datadir",
    long_about = "Export one or more Ethereum accounts' complete persistent state at an exact \
canonical block, directly from a Reth datadir. The database is opened cooperatively and read-only, \
so the command can run alongside an active Reth node. No JSON-RPC, networking, or second node is \
used. Every requested account shares one anchor block and is written into a single v1 snapshot \
file.",
    next_line_help = true,
    after_help = "Output:\n  Snapshot JSON is written to stdout by default, or atomically to FILE with --output.\n  Progress and errors are always written to stderr.\n\nExamples:\n  One account at the latest canonical database block:\n    reth-contract-snapshot --datadir /path/to/reth 0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48 > snapshot.json\n\n  Two accounts at a specific canonical block written directly to a file:\n    reth-contract-snapshot --datadir /path/to/reth --block 25891591 --output snapshot.json 0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48,0xdac17f958d2ee523a2206206994597c13d831ec7"
)]
pub(crate) struct Cli {
    /// Reth datadir to open cooperatively in read-only mode.
    ///
    /// The directory must contain the database, static files, and state-range data used by the
    /// running node.
    #[arg(
        short = 'd',
        long,
        value_name = "RETH_DATADIR",
        help_heading = "Node access"
    )]
    pub(crate) datadir: PathBuf,

    /// Comma-separated Ethereum contract or account addresses to snapshot into one file.
    ///
    /// Every address shares one anchor block. Storage keys in the resulting JSON are keccak256
    /// hashes of raw EVM storage slots.
    #[arg(value_name = "CONTRACT_ADDRESSES", value_parser = parse_address, value_delimiter = ',', required = true)]
    pub(crate) addresses: Vec<Address>,

    /// Canonical database block number; defaults to the latest available block.
    ///
    /// Accepts decimal and 0x-prefixed hexadecimal values. The selected state root must still be
    /// retained by Reth's state-range provider.
    #[arg(
        short = 'b',
        long,
        value_parser = parse_block_number,
        value_name = "BLOCK_NUMBER",
        help_heading = "Snapshot selection"
    )]
    pub(crate) block: Option<u64>,

    /// Write the completed snapshot to a file instead of stdout.
    ///
    /// The destination is replaced atomically only after storage exhaustion and root validation.
    #[arg(short = 'o', long, value_name = "FILE", help_heading = "Output")]
    pub(crate) output: Option<PathBuf>,

    /// Approximate response-byte limit for each bounded storage-range request.
    ///
    /// This controls per-page memory use, not the final snapshot size.
    #[arg(
        long,
        default_value_t = DEFAULT_PAGE_BYTES,
        value_name = "BYTES",
        help_heading = "Pagination"
    )]
    pub(crate) page_bytes: usize,
}

impl Cli {
    pub(crate) fn parse_args() -> Self {
        Self::parse()
    }
}

fn parse_address(value: &str) -> Result<Address, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("address list contains an empty entry".to_owned());
    }
    value
        .parse()
        .map_err(|_| format!("invalid Ethereum address: {value}"))
}

fn parse_block_number(value: &str) -> Result<u64, BlockNumberParseError> {
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        if hex.is_empty() {
            return Err(BlockNumberParseError::EmptyHex);
        }
        return u64::from_str_radix(hex, 16).map_err(BlockNumberParseError::InvalidHex);
    }
    value
        .parse::<u64>()
        .map_err(BlockNumberParseError::InvalidDecimal)
}

#[cfg(test)]
mod tests {
    use super::{Cli, parse_block_number};
    use alloy_primitives::Address;
    use clap::Parser;
    use std::path::PathBuf;

    #[test]
    fn parses_decimal_and_hex_block_numbers() {
        assert_eq!(parse_block_number("25891591").unwrap(), 25_891_591);
        assert_eq!(parse_block_number("0x18b15a7").unwrap(), 25_892_263);
        assert!(parse_block_number("latest").is_err());
        assert!(parse_block_number("0x").is_err());
    }

    #[test]
    fn parses_output_path() {
        let cli = Cli::try_parse_from([
            "reth-contract-snapshot",
            "--datadir",
            "/reth",
            "--output",
            "snapshot.json",
            "0x0000000000000000000000000000000000000001",
        ])
        .unwrap();
        assert_eq!(cli.output, Some(PathBuf::from("snapshot.json")));
    }

    #[test]
    fn parses_comma_separated_addresses() {
        let cli = Cli::try_parse_from([
            "reth-contract-snapshot",
            "--datadir",
            "/reth",
            "0x0000000000000000000000000000000000000001,0x0000000000000000000000000000000000000002",
        ])
        .unwrap();
        assert_eq!(
            cli.addresses,
            vec![Address::with_last_byte(1), Address::with_last_byte(2)]
        );
        assert!(
            Cli::try_parse_from([
                "reth-contract-snapshot",
                "--datadir",
                "/reth",
                "not-an-address",
            ])
            .is_err()
        );
        assert!(Cli::try_parse_from(["reth-contract-snapshot", "--datadir", "/reth"]).is_err());
    }
}
