//! QDOT Token - PolkaVM pallet-revive Smart Contract
//! Built for Polkadot Hub TestNet / Asset Hub Revive

#![no_main]
#![no_std]

use pallet_revive_uapi::{HostFn, HostFnImpl as api, ReturnFlags, StorageFlags};

// Function selectors (Keccak-256 4-byte hashes)
const BALANCE_OF_SELECTOR: [u8; 4] = [0x70, 0xa0, 0x82, 0x31]; // balanceOf(address)
const MINT_SELECTOR: [u8; 4] = [0x40, 0xc1, 0x0f, 0x19]; // mint(address,uint256)
const TOTAL_SUPPLY_SELECTOR: [u8; 4] = [0x18, 0x16, 0x0d, 0xdd]; // totalSupply()
const TRANSFER_SELECTOR: [u8; 4] = [0xa9, 0x05, 0x9c, 0xbb]; // transfer(address,uint256)
const BURN_SELECTOR: [u8; 4] = [0x42, 0x96, 0x6c, 0x68]; // burn(uint256)

// Event signatures
const TRANSFER_EVENT_SIGNATURE: [u8; 32] = [
    0xdd, 0xf2, 0x52, 0xad, 0x1b, 0xe2, 0xc8, 0x9b, 0x69, 0xc2, 0xb0, 0x68, 0xfc, 0x37, 0x8d, 0xaa,
    0x95, 0x2b, 0xa7, 0xf1, 0x63, 0xc4, 0xa1, 0x16, 0x28, 0xf5, 0x5a, 0x4d, 0xf5, 0x23, 0xb3, 0xef,
];

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe {
        core::arch::asm!("unimp");
        core::hint::unreachable_unchecked();
    }
}

/// Constructor called once on pallet-revive contract deployment
#[polkavm_derive::polkavm_export]
pub extern "C" fn deploy() {
    let caller = get_caller();
    // Default initial supply: 1,000,000 QDOT (1e24 with 18 decimals, or 1e12 base units)
    let initial_supply: u128 = 1_000_000_000_000_000_000;
    set_balance(&caller, initial_supply);
    set_total_supply(initial_supply);
    let zero_addr = [0u8; 20];
    emit_transfer(&zero_addr, &caller, initial_supply);
}

/// Main entry point dispatched on contract calls
#[polkavm_derive::polkavm_export]
pub extern "C" fn call() {
    let call_data_len = api::call_data_size() as usize;
    let mut call_data = [0u8; 256];
    if call_data_len > call_data.len() {
        panic!("Call data too large");
    }

    api::call_data_copy(&mut call_data[..call_data_len], 0);

    if call_data_len < 4 {
        panic!("Call data too short");
    }

    let selector: [u8; 4] = call_data[0..4].try_into().unwrap();

    match selector {
        BALANCE_OF_SELECTOR => {
            if call_data_len < 36 {
                panic!("Invalid balanceOf call data");
            }
            let account = decode_address(&call_data[4..36]);
            let balance = get_balance(&account);
            let output = to_word(balance);
            api::return_value(ReturnFlags::empty(), &output);
        }

        TOTAL_SUPPLY_SELECTOR => {
            let output = to_word(get_total_supply());
            api::return_value(ReturnFlags::empty(), &output);
        }

        TRANSFER_SELECTOR => {
            if call_data_len < 68 {
                panic!("Invalid transfer call data");
            }
            let to = decode_address(&call_data[4..36]);
            let amount = decode_u128(&call_data[36..68]);

            let caller = get_caller();
            let sender_balance = get_balance(&caller);
            if sender_balance < amount {
                panic!("InsufficientBalance");
            }

            let new_sender_balance = sender_balance - amount;
            let recipient_balance = get_balance(&to);
            let new_recipient_balance = recipient_balance.saturating_add(amount);

            set_balance(&caller, new_sender_balance);
            set_balance(&to, new_recipient_balance);
            emit_transfer(&caller, &to, amount);

            let ret_ok = [0u8; 32];
            api::return_value(ReturnFlags::empty(), &ret_ok);
        }

        MINT_SELECTOR => {
            if call_data_len < 68 {
                panic!("Invalid mint call data");
            }
            let to = decode_address(&call_data[4..36]);
            let amount = decode_u128(&call_data[36..68]);

            let new_balance = get_balance(&to).saturating_add(amount);
            set_balance(&to, new_balance);

            let new_supply = get_total_supply().saturating_add(amount);
            set_total_supply(new_supply);

            let zero_addr = [0u8; 20];
            emit_transfer(&zero_addr, &to, amount);

            let ret_ok = [0u8; 32];
            api::return_value(ReturnFlags::empty(), &ret_ok);
        }

        BURN_SELECTOR => {
            if call_data_len < 36 {
                panic!("Invalid burn call data");
            }
            let amount = decode_u128(&call_data[4..36]);
            let caller = get_caller();
            let balance = get_balance(&caller);
            if balance < amount {
                panic!("InsufficientBalance");
            }

            set_balance(&caller, balance - amount);
            let new_supply = get_total_supply().saturating_sub(amount);
            set_total_supply(new_supply);

            let zero_addr = [0u8; 20];
            emit_transfer(&caller, &zero_addr, amount);

            let ret_ok = [0u8; 32];
            api::return_value(ReturnFlags::empty(), &ret_ok);
        }

        _ => panic!("Unknown function selector"),
    }
}

// Storage layout helpers
#[inline(always)]
fn total_supply_key() -> [u8; 32] {
    [0u8; 32] // Slot 0
}

fn balance_key(addr: &[u8; 20]) -> [u8; 32] {
    let mut input = [0u8; 64];
    input[12..32].copy_from_slice(addr);
    input[63] = 1; // Slot 1
    let mut key = [0u8; 32];
    api::hash_keccak_256(&input, &mut key);
    key
}

fn get_total_supply() -> u128 {
    let key = total_supply_key();
    let mut supply_bytes = [0u8; 16];
    let mut slice = &mut supply_bytes[..];
    match api::get_storage(StorageFlags::empty(), &key, &mut slice) {
        Ok(_) => u128::from_be_bytes(supply_bytes),
        Err(_) => 0,
    }
}

fn set_total_supply(amount: u128) {
    let key = total_supply_key();
    let bytes = amount.to_be_bytes();
    api::set_storage(StorageFlags::empty(), &key, &bytes);
}

fn get_balance(addr: &[u8; 20]) -> u128 {
    let key = balance_key(addr);
    let mut balance_bytes = [0u8; 16];
    let mut slice = &mut balance_bytes[..];
    match api::get_storage(StorageFlags::empty(), &key, &mut slice) {
        Ok(_) => u128::from_be_bytes(balance_bytes),
        Err(_) => 0,
    }
}

fn set_balance(addr: &[u8; 20], amount: u128) {
    let key = balance_key(addr);
    let bytes = amount.to_be_bytes();
    api::set_storage(StorageFlags::empty(), &key, &bytes);
}

fn get_caller() -> [u8; 20] {
    let mut caller = [0u8; 20];
    api::caller(&mut caller);
    caller
}

fn emit_transfer(from: &[u8; 20], to: &[u8; 20], amount: u128) {
    let mut topic1 = [0u8; 32];
    topic1[12..32].copy_from_slice(from);

    let mut topic2 = [0u8; 32];
    topic2[12..32].copy_from_slice(to);

    let topics = [TRANSFER_EVENT_SIGNATURE, topic1, topic2];
    let data = to_word(amount);

    api::deposit_event(&topics, &data);
}

#[inline(always)]
fn to_word(val: u128) -> [u8; 32] {
    let mut word = [0u8; 32];
    word[16..32].copy_from_slice(&val.to_be_bytes());
    word
}

#[inline(always)]
fn decode_address(slice: &[u8]) -> [u8; 20] {
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&slice[12..32]);
    addr
}

#[inline(always)]
fn decode_u128(slice: &[u8]) -> u128 {
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&slice[16..32]);
    u128::from_be_bytes(bytes)
}
