# arroyo-deployer — one-shot pipeline deployer for Railway.
#
# Uses the official Arroyo image (has bash, python3, curl) to run
# deploy-pipelines.sh which submits SQL pipeline definitions to the
# Arroyo REST API and substitutes env-var hostnames at deploy time.
FROM ghcr.io/arroyosystems/arroyo:latest

COPY infra/arroyo/pipelines /pipelines
COPY infra/arroyo/deploy-pipelines.sh /usr/local/bin/deploy-pipelines.sh

RUN chmod +x /usr/local/bin/deploy-pipelines.sh

ENTRYPOINT ["/bin/bash", "/usr/local/bin/deploy-pipelines.sh"]
