//! Qmoosa Launchpad - PolkaVM pallet-revive Smart Contract
//! Built for Polkadot Hub TestNet / Asset Hub Revive

#![no_main]
#![no_std]

use pallet_revive_uapi::{HostFn, HostFnImpl as api, ReturnFlags, StorageFlags};

// Function selectors
const BUY_TOKENS_SELECTOR: [u8; 4] = [0xd9, 0x6a, 0x09, 0x4a]; // buyTokens()
const CLAIM_TOKENS_SELECTOR: [u8; 4] = [0xdf, 0x13, 0x34, 0x41]; // claimTokens()
const GET_CONTRIBUTED_SELECTOR: [u8; 4] = [0x01, 0x7e, 0x40, 0x93]; // getContributed(address)
const TOTAL_RAISED_SELECTOR: [u8; 4] = [0xbb, 0x6e, 0x82, 0x22]; // totalRaised()

// Event signatures
const TOKENS_PURCHASED_EVENT_SIGNATURE: [u8; 32] = [
    0x4d, 0xf2, 0x52, 0xad, 0x1b, 0xe2, 0xc8, 0x9b, 0x69, 0xc2, 0xb0, 0x68, 0xfc, 0x37, 0x8d, 0xaa,
    0x95, 0x2b, 0xa7, 0xf1, 0x63, 0xc4, 0xa1, 0x16, 0x28, 0xf5, 0x5a, 0x4d, 0xf5, 0x23, 0xb3, 0x01,
];

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe {
        core::arch::asm!("unimp");
        core::hint::unreachable_unchecked();
    }
}

#[polkavm_derive::polkavm_export]
pub extern "C" fn deploy() {
    let caller = get_caller();
    set_owner(&caller);
}

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
        BUY_TOKENS_SELECTOR => {
            let caller = get_caller();
            let mut value_bytes = [0u8; 32];
            api::value_transferred(&mut value_bytes);
            let mut amount_bytes = [0u8; 16];
            amount_bytes.copy_from_slice(&value_bytes[16..32]);
            let amount = u128::from_be_bytes(amount_bytes);

            let contributed = get_contributed(&caller).saturating_add(amount);
            set_contributed(&caller, contributed);

            let total = get_total_raised().saturating_add(amount);
            set_total_raised(total);

            emit_purchase(&caller, amount);

            let ret_ok = [0u8; 32];
            api::return_value(ReturnFlags::empty(), &ret_ok);
        }

        CLAIM_TOKENS_SELECTOR => {
            let caller = get_caller();
            let contributed = get_contributed(&caller);
            if contributed == 0 {
                panic!("No tokens to claim");
            }
            set_contributed(&caller, 0);

            let ret_ok = [0u8; 32];
            api::return_value(ReturnFlags::empty(), &ret_ok);
        }

        GET_CONTRIBUTED_SELECTOR => {
            if call_data_len < 36 {
                panic!("Invalid getContributed data");
            }
            let account = decode_address(&call_data[4..36]);
            let contributed = get_contributed(&account);
            let output = to_word(contributed);
            api::return_value(ReturnFlags::empty(), &output);
        }

        TOTAL_RAISED_SELECTOR => {
            let total = get_total_raised();
            let output = to_word(total);
            api::return_value(ReturnFlags::empty(), &output);
        }

        _ => panic!("Unknown selector"),
    }
}

// Storage helpers
fn total_raised_key() -> [u8; 32] {
    [0u8; 32]
}

fn owner_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    key[31] = 1;
    key
}

fn contributed_key(addr: &[u8; 20]) -> [u8; 32] {
    let mut input = [0u8; 64];
    input[12..32].copy_from_slice(addr);
    input[63] = 2; // Slot 2
    let mut key = [0u8; 32];
    api::hash_keccak_256(&input, &mut key);
    key
}

fn get_total_raised() -> u128 {
    let key = total_raised_key();
    let mut bytes = [0u8; 16];
    let mut slice = &mut bytes[..];
    match api::get_storage(StorageFlags::empty(), &key, &mut slice) {
        Ok(_) => u128::from_be_bytes(bytes),
        Err(_) => 0,
    }
}

fn set_total_raised(amount: u128) {
    let key = total_raised_key();
    let bytes = amount.to_be_bytes();
    api::set_storage(StorageFlags::empty(), &key, &bytes);
}

fn set_owner(addr: &[u8; 20]) {
    let key = owner_key();
    api::set_storage(StorageFlags::empty(), &key, addr);
}

fn get_contributed(addr: &[u8; 20]) -> u128 {
    let key = contributed_key(addr);
    let mut bytes = [0u8; 16];
    let mut slice = &mut bytes[..];
    match api::get_storage(StorageFlags::empty(), &key, &mut slice) {
        Ok(_) => u128::from_be_bytes(bytes),
        Err(_) => 0,
    }
}

fn set_contributed(addr: &[u8; 20], amount: u128) {
    let key = contributed_key(addr);
    let bytes = amount.to_be_bytes();
    api::set_storage(StorageFlags::empty(), &key, &bytes);
}

fn get_caller() -> [u8; 20] {
    let mut caller = [0u8; 20];
    api::caller(&mut caller);
    caller
}

fn emit_purchase(buyer: &[u8; 20], amount: u128) {
    let mut topic1 = [0u8; 32];
    topic1[12..32].copy_from_slice(buyer);
    let topics = [TOKENS_PURCHASED_EVENT_SIGNATURE, topic1];
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
