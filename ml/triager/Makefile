.PHONY: train calibrate novelty all clean

ARTIFACTS := artifacts
PYTHON := python3

all: train

train: $(ARTIFACTS)/model.onnx $(ARTIFACTS)/shap_background.npy

$(ARTIFACTS)/model.onnx: golden_cases.json train.py
	$(PYTHON) train.py

$(ARTIFACTS)/novelty_detector.onnx: golden_cases.json novelty.py
	$(PYTHON) novelty.py

novelty: $(ARTIFACTS)/novelty_detector.onnx

calibrate: train
	$(PYTHON) calibrate.py --train

serve-calibration:
	$(PYTHON) calibrate.py --serve

all: $(ARTIFACTS)/model.onnx $(ARTIFACTS)/novelty_detector.onnx
	$(PYTHON) calibrate.py --train
	@echo "All artifacts built in $(ARTIFACTS)/"

clean:
	rm -rf $(ARTIFACTS)/*.onnx $(ARTIFACTS)/*.npy $(ARTIFACTS)/*.pkl $(ARTIFACTS)/*.txt $(ARTIFACTS)/*.json
