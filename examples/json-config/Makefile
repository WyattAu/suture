.PHONY: all clean

SUTURE ?= $(shell command -v suture 2>/dev/null || echo "../target/debug/suture")

REPO := /tmp/suture-example-json-config

all: $(SUTURE)
	@echo "=== JSON Cloud Config Versioning Demo ==="
	@echo ""
	@rm -rf $(REPO)
	@mkdir -p $(REPO)
	@cd $(REPO) && \
		echo "--- Step 1: Initialize repo and commit base config ---" && \
		$(SUTURE) init . && \
		$(SUTURE) config user.name="DevOps Team" && \
		$(SUTURE) config user.email="devops@example.com" && \
		cp $(CURDIR)/config.json config.json && \
		$(SUTURE) add config.json && \
		$(SUTURE) commit "Initial cloud infrastructure config" && \
		echo "" && \
		echo "--- Step 2: Create feature-a branch (add new subnet) ---" && \
		$(SUTURE) branch feature-a && \
		$(SUTURE) checkout feature-a && \
		cp $(CURDIR)/team/feature-a/config.json config.json && \
		$(SUTURE) add config.json && \
		$(SUTURE) commit "Add private-subnet-b in us-east-1b" && \
		echo "" && \
		echo "--- Step 3: Create feature-b branch (update security group) ---" && \
		$(SUTURE) checkout main && \
		$(SUTURE) branch feature-b && \
		$(SUTURE) checkout feature-b && \
		cp $(CURDIR)/team/feature-b/config.json config.json && \
		$(SUTURE) add config.json && \
		$(SUTURE) commit "Add SSH ingress rule from bastion to web-sg" && \
		echo "" && \
		echo "--- Step 4: Merge feature-a into main ---" && \
		$(SUTURE) checkout main && \
		$(SUTURE) merge feature-a && \
		echo "" && \
		echo "--- Step 5: Merge feature-b into main ---" && \
		$(SUTURE) merge feature-b && \
		echo "" && \
		echo "--- Step 6: Show log ---" && \
		$(SUTURE) log --oneline --graph && \
		echo "" && \
		echo "--- Step 7: Diff main against base ---" && \
		$(SUTURE) diff --from HEAD~3 --to HEAD && \
		echo "" && \
		echo "--- Done: Both changes merged cleanly ---"
	@rm -rf $(REPO)

clean:
	rm -rf $(REPO)
