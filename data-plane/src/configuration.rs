#[cfg(feature = "enclave")]
pub fn get_cert_provisioner_host() -> String {
    "provisioner.cages.internal".to_string()
}

#[cfg(not(feature = "enclave"))]
pub fn get_cert_provisioner_host() -> String {
    "localhost".to_string()
}

#[cfg(feature = "enclave")]
pub fn get_e3_host() -> String {
    "e3.cages-e3.internal".to_string()
}

#[cfg(not(feature = "enclave"))]
pub fn get_e3_host() -> String {
    "localhost".to_string()
}

pub fn get_egress_ports() -> Vec<u16> {
    let port_str = std::env::var("EGRESS_PORTS").unwrap_or("443".to_string());
    port_str
        .split(',')
        .map(|port| {
            port.parse::<u16>()
                .unwrap_or_else(|_| panic!("Could not parse egress port as u16: {port}"))
        })
        .collect()
}

pub fn get_egress_allow_list() -> Vec<String> {
    let domain_str = std::env::var("EGRESS_ALLOW_LIST").unwrap_or("".to_string());
    domain_str
        .split(',')
        .map(|domain| domain.to_string())
        .collect()
}
