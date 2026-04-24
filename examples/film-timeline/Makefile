.PHONY: all clean

SUTURE ?= $(shell command -v suture 2>/dev/null || echo "../target/debug/suture")

REPO := /tmp/suture-example-film-timeline

all: $(SUTURE)
	@echo "=== Film Timeline Versioning Demo ==="
	@echo ""
	@rm -rf $(REPO)
	@mkdir -p $(REPO)
	@cd $(REPO) && \
		echo "--- Step 1: Initialize repo and commit base timeline ---" && \
		$(SUTURE) init . && \
		$(SUTURE) config user.name="Marcus Rivera" && \
		$(SUTURE) config user.email="marcus@postprod.example.com" && \
		cp $(CURDIR)/timeline.otio timeline.otio && \
		$(SUTURE) add timeline.otio && \
		$(SUTURE) commit "Rough cut: 5 scenes, 4800 frames" && \
		echo "" && \
		echo "--- Step 2: Create v1-review branch with director notes ---" && \
		$(SUTURE) branch v1-review && \
		$(SUTURE) checkout v1-review && \
		mkdir -p notes && \
		cp $(CURDIR)/v1/notes.md notes/v1-notes.md && \
		$(SUTURE) add notes/v1-notes.md && \
		$(SUTURE) commit "Add director notes for v1 screening" && \
		echo "" && \
		echo "--- Step 3: Create v2-edit branch with scene reorder ---" && \
		$(SUTURE) checkout main && \
		$(SUTURE) branch v2-edit && \
		$(SUTURE) checkout v2-edit && \
		python3 -c "
import json
with open('timeline.otio') as f: tl = json.load(f)
clips = tl['tracks'][0]['children']
clips[2], clips[3] = clips[3], clips[2]
tl['name'] = 'The Last Horizon - v2 Cut'
with open('timeline.otio', 'w') as f: json.dump(tl, f, indent=2)
" && \
		$(SUTURE) add timeline.otio && \
		$(SUTURE) commit "v2 edit: reorder storm and calm scenes" && \
		mkdir -p notes && \
		cp $(CURDIR)/v2/notes.md notes/v2-notes.md && \
		$(SUTURE) add notes/v2-notes.md && \
		$(SUTURE) commit "Add director notes for v2 screening" && \
		echo "" && \
		echo "--- Step 4: Tag v1 and v2 on main ---" && \
		$(SUTURE) checkout main && \
		$(SUTURE) merge v1-review && \
		$(SUTURE) tag v1.0 && \
		$(SUTURE) merge v2-edit && \
		$(SUTURE) tag v2.0 && \
		echo "" && \
		echo "--- Step 5: Show diff between v1.0 and v2.0 ---" && \
		$(SUTURE) diff --from v1.0 --to v2.0 && \
		echo "" && \
		echo "--- Step 6: Show log with graph ---" && \
		$(SUTURE) log --oneline --graph && \
		echo "" && \
		echo "--- Done: Timeline versioned with tags ---"
	@rm -rf $(REPO)

clean:
	rm -rf $(REPO)
