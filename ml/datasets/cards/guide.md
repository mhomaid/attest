# guide

## Source

GUIDE (Microsoft Security Incident Prediction), the public incident corpus released with Freitas, Kalajdjieski, Gharib, and McCann, arXiv:2407.09017. Train and test CSV files are published at https://www.kaggle.com/datasets/Microsoft/microsoft-security-incident-prediction

## License

CDLA-Permissive-2.0 (the paper describes the release as permissive CDLA-2.0). This registry allows train and eval.

## Size

About 1 million incidents with analyst triage labels, 1.6 million alerts, and 13 million evidence rows, drawn from roughly 6,100 organizations over a two-week window. The publisher splits the rows 70/30 into train and test CSV files.

## Labels

Incident triage grade assigned by customer security analysts: true positive, benign positive, or false positive. A smaller slice of alerts also carries a remediation action.

## Known biases

The sample is one cloud region and a two-week window, so organization size, detector mix, and product coverage are uneven. Labels follow that period's analyst practice. Identifiers are anonymized and timestamps are noised. The publisher has not published a file checksum; `make dataset-fetch` will not keep bytes until a sha256 or md5 digest is pinned in the registry. This card contains no dataset rows.
