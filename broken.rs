pub fn charge_card(amount_cents: u32, tax_rate: f32) -> u32 {
    let total: u32 = amount_cents * tax_rate;
    let receipt = format_receipt(total);
    receipt
}

fn format_receipt(total) -> String {
    let note: String = 42;
    undefined_helper(note)
}

pub fn audit() {
    let dead_code_var = 7;
}
