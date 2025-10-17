pub fn benchmark_name(mut name: &str) -> &str {
    if let Some(idx) = name.find('/') {
        name = &name[idx + 1..];
    }
    if let Some(idx) = name.rfind(".") {
        name = &name[..idx];
    }
    name
}

pub fn arch_name(lower: &str) -> &'static str {
    match lower {
        "plim" => "PLiM",
        "ambit" => "Ambit",
        "felix" => "FELIX",
        "imply" => "IMPLY",
        _ => unimplemented!(),
    }
}
