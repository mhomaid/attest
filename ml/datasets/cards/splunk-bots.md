# splunk-bots

## Source

Splunk Boss of the SOC dataset, version 2, full pre-indexed archive: https://s3.amazonaws.com/botsdataset/botsv2/botsv2_data_set.tgz

The publisher documents the archive and its MD5 digest at https://github.com/splunk/botsv2 (md5:fd2673726c96e97a39fc03119d6686c6).

## License

CC0-1.0. The license would allow broad reuse. This registry still allows eval only: the corpus is a scripted exercise with an answer key, not a source of production model weights.

## Size

16.4 GB compressed. The publisher also offers a 3.2 GB attack-only archive; this entry pins the full set.

## Labels

There is no per-event class column. Ground truth for evaluation is the published question and answer set used to score an investigator.

## Known biases

The traffic is a designed capture-the-flag exercise. Benign periods are staged next to attacks, and the archive is pre-indexed, so the on-disk layout depends on the Splunk version used to package it. Do not train production models on it. This card contains no events and no answer key.
