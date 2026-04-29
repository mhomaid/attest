# Calibration sidecar — Python Flask service that serves isotonic regression
# calibration models trained by ml/triager/calibrate.py.

FROM python:3.12-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    gcc \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

RUN pip install --no-cache-dir flask numpy scikit-learn

COPY ml/triager/calibrate.py ./calibrate.py
COPY ml/triager/artifacts/ ./artifacts/

ENV CALIBRATION_PORT=5001
ENV ARTIFACTS_DIR=/app/artifacts
ENV PYTHONUNBUFFERED=1

EXPOSE 5001

CMD ["python", "calibrate.py", "--serve"]
