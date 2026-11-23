# rba-login

## Source

Login Data Set for Risk-Based Authentication, Zenodo record 6782156: https://zenodo.org/records/6782156

The file this registry fetches is `rba-dataset.zip`, checksum md5:cc1b1078b3929650e6c08678caffcc57, as published on the Zenodo record.

## License

CC-BY-4.0. Cite Wiefling, Jørgensen, Thunem, and Lo Iacono, ACM Transactions on Privacy and Security, 2022, doi:10.1145/3546069. This registry allows train and eval.

## Size

More than 33 million login attempts and more than 3.3 million users. The zip is 1,093,700,330 bytes. The CSV inside is about 9.1 GB.

## Labels

Each row is one login attempt. Labels are whether the login succeeded, whether the IP appeared on a known-attacker list, and whether incident response marked the attempt as an account takeover.

## Known biases

The published features are synthesized so that summary statistics match a real large-scale SSO. The authors warn that the values are artificial and should not be treated as that service's production logs. Geography, user agents, and latency follow one national-scale service, not a global population. Attacker-IP and takeover labels depend on that service's lists and incident response. This card contains no dataset rows.
