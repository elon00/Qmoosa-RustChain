//! x402 Settlement - PolkaVM pallet-revive Smart Contract
//! Built for Polkadot Hub TestNet / Asset Hub Revive

#![no_main]
#![no_std]

use pallet_revive_uapi::{HostFn, HostFnImpl as api, ReturnFlags, StorageFlags};

// Function selectors
const SETTLE_PAYMENT_SELECTOR: [u8; 4] = [0x5f, 0x1d, 0x48, 0x93]; // settlePayment(bytes32,uint256,address)
const IS_SETTLED_SELECTOR: [u8; 4] = [0x8b, 0x5a, 0x8a, 0x22]; // isSettled(bytes32)
const TOTAL_SETTLED_SELECTOR: [u8; 4] = [0x3c, 0xb8, 0x56, 0x11]; // totalSettled()

// Event signatures
const PAYMENT_SETTLED_EVENT_SIGNATURE: [u8; 32] = [
    0x8e, 0xf2, 0x52, 0xad, 0x1b, 0xe2, 0xc8, 0x9b, 0x69, 0xc2, 0xb0, 0x68, 0xfc, 0x37, 0x8d, 0xaa,
    0x95, 0x2b, 0xa7, 0xf1, 0x63, 0xc4, 0xa1, 0x16, 0x28, 0xf5, 0x5a, 0x4d, 0xf5, 0x23, 0xb3, 0x99,
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
        SETTLE_PAYMENT_SELECTOR => {
            if call_data_len < 100 {
                panic!("Invalid settlePayment call data");
            }
            let mut tx_hash = [0u8; 32];
            tx_hash.copy_from_slice(&call_data[4..36]);
            let amount = decode_u128(&call_data[36..68]);
            let payer = decode_address(&call_data[68..100]);

            if is_settled(&tx_hash) {
                panic!("Payment already settled (replay protected)");
            }

            mark_settled(&tx_hash);
            let total = get_total_settled().saturating_add(amount);
            set_total_settled(total);

            emit_settlement(&tx_hash, &payer, amount);

            let ret_ok = [0u8; 32];
            api::return_value(ReturnFlags::empty(), &ret_ok);
        }

        IS_SETTLED_SELECTOR => {
            if call_data_len < 36 {
                panic!("Invalid isSettled call data");
            }
            let mut tx_hash = [0u8; 32];
            tx_hash.copy_from_slice(&call_data[4..36]);
            let settled = is_settled(&tx_hash);
            let mut output = [0u8; 32];
            if settled {
                output[31] = 1;
            }
            api::return_value(ReturnFlags::empty(), &output);
        }

        TOTAL_SETTLED_SELECTOR => {
            let total = get_total_settled();
            let output = to_word(total);
            api::return_value(ReturnFlags::empty(), &output);
        }

        _ => panic!("Unknown selector"),
    }
}

// Storage helpers
fn total_settled_key() -> [u8; 32] {
    [0u8; 32]
}

fn owner_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    key[31] = 1;
    key
}

fn settlement_record_key(tx_hash: &[u8; 32]) -> [u8; 32] {
    let mut input = [0u8; 64];
    input[..32].copy_from_slice(tx_hash);
    input[63] = 2; // Slot 2
    let mut key = [0u8; 32];
    api::hash_keccak_256(&input, &mut key);
    key
}

fn is_settled(tx_hash: &[u8; 32]) -> bool {
    let key = settlement_record_key(tx_hash);
    let mut val = [0u8; 1];
    let mut slice = &mut val[..];
    match api::get_storage(StorageFlags::empty(), &key, &mut slice) {
        Ok(_) => val[0] == 1,
        Err(_) => false,
    }
}

fn mark_settled(tx_hash: &[u8; 32]) {
    let key = settlement_record_key(tx_hash);
    let val = [1u8];
    api::set_storage(StorageFlags::empty(), &key, &val);
}

fn get_total_settled() -> u128 {
    let key = total_settled_key();
    let mut bytes = [0u8; 16];
    let mut slice = &mut bytes[..];
    match api::get_storage(StorageFlags::empty(), &key, &mut slice) {
        Ok(_) => u128::from_be_bytes(bytes),
        Err(_) => 0,
    }
}

fn set_total_settled(amount: u128) {
    let key = total_settled_key();
    let bytes = amount.to_be_bytes();
    api::set_storage(StorageFlags::empty(), &key, &bytes);
}

fn set_owner(addr: &[u8; 20]) {
    let key = owner_key();
    api::set_storage(StorageFlags::empty(), &key, addr);
}

fn get_caller() -> [u8; 20] {
    let mut caller = [0u8; 20];
    api::caller(&mut caller);
    caller
}

fn emit_settlement(tx_hash: &[u8; 32], payer: &[u8; 20], amount: u128) {
    let mut topic2 = [0u8; 32];
    topic2[12..32].copy_from_slice(payer);
    let topics = [PAYMENT_SETTLED_EVENT_SIGNATURE, *tx_hash, topic2];
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
