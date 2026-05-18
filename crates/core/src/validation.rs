use std::net::{IpAddr, ToSocketAddrs};

use url::{Host, Url};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedUrl {
    pub normalized_url: String,
    pub url: Url,
}

pub fn validate_scan_url(input: &str) -> Result<ValidatedUrl, UrlValidationError> {
    validate_scan_url_with_resolver(input, |host, port| {
        (host, port)
            .to_socket_addrs()
            .map(|addrs| addrs.map(|addr| addr.ip()).collect())
            .map_err(UrlValidationError::ResolveFailed)
    })
}

pub fn validate_scan_url_with_resolver<F>(
    input: &str,
    resolver: F,
) -> Result<ValidatedUrl, UrlValidationError>
where
    F: Fn(&str, u16) -> Result<Vec<IpAddr>, UrlValidationError>,
{
    let url = Url::parse(input).map_err(UrlValidationError::InvalidUrl)?;

    match url.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(UrlValidationError::UnsupportedScheme {
                scheme: scheme.to_owned(),
            });
        }
    }

    let host = url.host().ok_or(UrlValidationError::EmptyHost)?;
    let port = url.port_or_known_default().ok_or(UrlValidationError::UnknownPort)?;

    match host {
        Host::Ipv4(ip) => reject_ip(IpAddr::V4(ip))?,
        Host::Ipv6(ip) => reject_ip(IpAddr::V6(ip))?,
        Host::Domain(domain) => {
            if domain.trim().is_empty() {
                return Err(UrlValidationError::EmptyHost);
            }

            let resolved = resolver(domain, port)?;

            if resolved.is_empty() {
                return Err(UrlValidationError::NoResolvedAddresses {
                    host: domain.to_owned(),
                });
            }

            for ip in resolved {
                reject_ip(ip)?;
            }
        }
    }

    Ok(ValidatedUrl {
        normalized_url: url.to_string(),
        url,
    })
}

fn reject_ip(ip: IpAddr) -> Result<(), UrlValidationError> {
    if is_disallowed_ip(ip) {
        return Err(UrlValidationError::DisallowedIp { ip });
    }

    Ok(())
}

fn is_disallowed_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_multicast()
                || ip.is_unspecified()
                || ip.octets()[0] == 0
        }
        IpAddr::V6(ip) => {
            ip.is_loopback()
                || ip.is_multicast()
                || ip.is_unspecified()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
        }
    }
}

#[derive(Debug)]
pub enum UrlValidationError {
    DisallowedIp { ip: IpAddr },
    EmptyHost,
    InvalidUrl(url::ParseError),
    NoResolvedAddresses { host: String },
    ResolveFailed(std::io::Error),
    UnknownPort,
    UnsupportedScheme { scheme: String },
}

impl std::fmt::Display for UrlValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DisallowedIp { ip } => write!(f, "resolved IP address {ip} is not allowed"),
            Self::EmptyHost => write!(f, "URL host must not be empty"),
            Self::InvalidUrl(error) => write!(f, "invalid URL: {error}"),
            Self::NoResolvedAddresses { host } => {
                write!(f, "hostname {host} did not resolve to any addresses")
            }
            Self::ResolveFailed(error) => write!(f, "failed to resolve hostname: {error}"),
            Self::UnknownPort => write!(f, "URL scheme does not have a known default port"),
            Self::UnsupportedScheme { scheme } => {
                write!(f, "unsupported URL scheme '{scheme}'")
            }
        }
    }
}

impl std::error::Error for UrlValidationError {}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    use super::{validate_scan_url_with_resolver, UrlValidationError};

    #[test]
    fn rejects_non_http_scheme() {
        let result = validate_scan_url_with_resolver("ftp://example.com", resolver([]));

        assert!(matches!(
            result,
            Err(UrlValidationError::UnsupportedScheme { scheme }) if scheme == "ftp"
        ));
    }

    #[test]
    fn accepts_public_ipv4_host() {
        let result = validate_scan_url_with_resolver("https://93.184.216.34/path", resolver([]));

        match result {
            Ok(validated) => {
                assert_eq!(validated.normalized_url, "https://93.184.216.34/path");
            }
            Err(error) => panic!("expected public IPv4 URL to pass validation: {error}"),
        }
    }

    #[test]
    fn rejects_private_ipv4_host() {
        let result = validate_scan_url_with_resolver("http://10.0.0.8", resolver([]));

        assert!(matches!(
            result,
            Err(UrlValidationError::DisallowedIp { ip }) if ip == IpAddr::V4(Ipv4Addr::new(10, 0, 0, 8))
        ));
    }

    #[test]
    fn rejects_loopback_ipv6_host() {
        let result = validate_scan_url_with_resolver("https://[::1]/", resolver([]));

        assert!(matches!(
            result,
            Err(UrlValidationError::DisallowedIp { ip }) if ip == IpAddr::V6(Ipv6Addr::LOCALHOST)
        ));
    }

    #[test]
    fn accepts_hostname_with_public_resolution() {
        let result = validate_scan_url_with_resolver(
            "https://example.com/scan-me",
            resolver([(
                "example.com",
                vec![IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))],
            )]),
        );

        match result {
            Ok(validated) => {
                assert_eq!(validated.url.host_str(), Some("example.com"));
            }
            Err(error) => panic!("expected hostname URL to pass validation: {error}"),
        }
    }

    #[test]
    fn rejects_hostname_with_private_resolution() {
        let result = validate_scan_url_with_resolver(
            "https://internal.example",
            resolver([(
                "internal.example",
                vec![IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10))],
            )]),
        );

        assert!(matches!(result, Err(UrlValidationError::DisallowedIp { .. })));
    }

    #[test]
    fn rejects_empty_host() {
        let result = validate_scan_url_with_resolver("https://", resolver([]));

        assert!(matches!(
            result,
            Err(UrlValidationError::InvalidUrl(_)) | Err(UrlValidationError::EmptyHost)
        ));
    }

    #[test]
    fn rejects_hostname_without_addresses() {
        let result =
            validate_scan_url_with_resolver("https://example.com", resolver([("example.com", vec![])]));

        assert!(matches!(
            result,
            Err(UrlValidationError::NoResolvedAddresses { host }) if host == "example.com"
        ));
    }

    fn resolver<const N: usize>(
        entries: [(&'static str, Vec<IpAddr>); N],
    ) -> impl Fn(&str, u16) -> Result<Vec<IpAddr>, UrlValidationError> {
        let mappings = BTreeMap::from(entries);

        move |host, _port| Ok(mappings.get(host).cloned().unwrap_or_default())
    }
}
