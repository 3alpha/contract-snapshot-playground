use alloy_primitives::{Address, keccak256};
use reth_contract_snapshot::{
    ContractStateSource, PageProgress, SnapshotAccount, SnapshotAnchor, SnapshotStats,
    provider::RethStateSource, write_accounts,
};
use std::{
    collections::BTreeSet,
    error::Error,
    io::{BufWriter, Seek, Write},
    path::{Path, PathBuf},
};

mod command;

use command::{AppError, Cli, StagedSnapshot};

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        let mut source = error.source();
        while let Some(cause) = source {
            eprintln!("caused by: {cause}");
            source = cause.source();
        }
        std::process::exit(1);
    }
}

fn run() -> Result<(), AppError> {
    let cli = Cli::parse_args();
    if cli.page_bytes == 0 {
        return Err(AppError::InvalidPageBytes);
    }
    validate_unique_addresses(&cli.addresses)?;

    eprintln!("opening Reth datadir read-only: {}", cli.datadir.display());
    let source = RethStateSource::open(&cli.datadir)?;
    let anchor = match cli.block {
        Some(block_number) => source.anchor_at(block_number)?,
        None => source.anchor()?,
    };
    print_anchor(&anchor, &cli.addresses);

    let mut accounts = Vec::with_capacity(cli.addresses.len());
    for address in &cli.addresses {
        let account = source.account(&anchor, *address)?;
        print_account(&account);
        accounts.push(account);
    }

    let (snapshot, stats, serialized_size) =
        prepare_snapshot(&source, &anchor, &accounts, cli.page_bytes, cli.output)?;
    let output = snapshot.commit()?;
    print_summary(stats, serialized_size, output.as_deref());
    Ok(())
}

fn validate_unique_addresses(addresses: &[Address]) -> Result<(), AppError> {
    let mut unique = BTreeSet::new();
    for address in addresses {
        if !unique.insert(*address) {
            return Err(AppError::DuplicateAddress(*address));
        }
    }
    Ok(())
}

fn prepare_snapshot(
    source: &RethStateSource,
    anchor: &SnapshotAnchor,
    accounts: &[SnapshotAccount],
    page_bytes: usize,
    output: Option<PathBuf>,
) -> Result<(StagedSnapshot, SnapshotStats, u64), AppError> {
    let mut snapshot = StagedSnapshot::new(output)?;

    eprintln!("\nstorage");
    let stats = {
        let mut writer = BufWriter::new(snapshot.file_mut());
        let stats = write_accounts(
            source,
            anchor,
            accounts,
            page_bytes,
            &mut writer,
            print_page_progress,
        )?;
        writer.flush()?;
        stats
    };

    let serialized_size = snapshot.file_mut().stream_position()?;
    Ok((snapshot, stats, serialized_size))
}

fn print_anchor(anchor: &SnapshotAnchor, addresses: &[Address]) {
    eprintln!("\nsnapshot anchor");
    eprintln!("  block:          {}", anchor.block.number);
    eprintln!("  block hash:     {}", anchor.block.hash);
    eprintln!("  state root:     {}", anchor.block.state_root);
    eprintln!("  hardfork:       {}", anchor.execution.hardfork);
    eprintln!("  chain ID:       {}", anchor.execution.chain_id);
    eprintln!("  accounts:       {}", addresses.len());
    for address in addresses {
        eprintln!("    {address} (hashed: {})", keccak256(address));
    }
}

fn print_account(account: &SnapshotAccount) {
    eprintln!("\naccount {}", account.address());
    eprintln!("  nonce:        {}", account.account.nonce);
    eprintln!("  balance:      {}", account.account.balance);
    eprintln!("  code size:    {} bytes", account.code.len());
    eprintln!("  storage root: {}", account.account.storage_root);
}

fn print_page_progress(progress: PageProgress) {
    eprintln!(
        "  page {:>4}: {:>8} entries | {:>10} total | {}",
        progress.page,
        progress.entries,
        progress.total_entries,
        progress.end.as_str()
    );
}

fn print_summary(stats: SnapshotStats, serialized_size: u64, output: Option<&Path>) {
    eprintln!("\nsnapshot complete");
    eprintln!("  pages:           {}", stats.pages);
    eprintln!("  storage entries: {}", stats.entries);
    eprintln!("  serialized size: {serialized_size} bytes");
    match output {
        Some(path) => eprintln!("  output:          {}", path.display()),
        None => eprintln!("  output:          stdout"),
    }
}

#[cfg(test)]
mod tests {
    use super::{AppError, validate_unique_addresses};
    use alloy_primitives::Address;

    #[test]
    fn rejects_repeated_cli_addresses() {
        let address = Address::repeat_byte(0x11);
        assert!(matches!(
            validate_unique_addresses(&[address, Address::repeat_byte(0x22), address]),
            Err(AppError::DuplicateAddress(duplicate)) if duplicate == address
        ));
    }
}
