.PHONY: all clean

SUTURE ?= $(shell command -v suture 2>/dev/null || echo "../target/debug/suture")

REPO := /tmp/suture-example-youtube-channel

all: $(SUTURE)
	@echo "=== YouTube Channel Metadata Management Demo ==="
	@echo ""
	@rm -rf $(REPO)
	@mkdir -p $(REPO)
	@cd $(REPO) && \
		echo "--- Step 1: Initialize repo and commit all metadata ---" && \
		$(SUTURE) init . && \
		$(SUTURE) config user.name="ByteForge Team" && \
		$(SUTURE) config user.email="team@byteforge.example.com" && \
		mkdir -p channel videos/video-001 videos/video-002 playlists && \
		cp $(CURDIR)/channel/metadata.json channel/metadata.json && \
		cp $(CURDIR)/videos/video-001/metadata.json videos/video-001/metadata.json && \
		cp $(CURDIR)/videos/video-002/metadata.json videos/video-002/metadata.json && \
		cp $(CURDIR)/playlists/gaming.json playlists/gaming.json && \
		$(SUTURE) add channel/ videos/ playlists/ && \
		$(SUTURE) commit "Initial channel setup with 2 videos" && \
		echo "" && \
		echo "--- Step 2: Create seo-update branch ---" && \
		$(SUTURE) branch seo-update && \
		$(SUTURE) checkout seo-update && \
		python3 -c "
import json
for vid in ['video-001', 'video-002']:
    path = f'videos/{vid}/metadata.json'
    with open(path) as f: data = json.load(f)
    data['tags'].extend(['godot 4.3', 'game dev 2025'])
    if vid == 'video-001':
        data['title'] = 'Building a 2D Platformer in Godot 4.3 (2025) — Complete Beginner Tutorial'
    else:
        data['title'] = 'Procedural Generation Deep Dive: Noise, Waves, and Infinite Worlds (2025)'
    with open(path, 'w') as f: json.dump(data, f, indent=2)
" && \
		$(SUTURE) add videos/ && \
		$(SUTURE) commit "SEO: update titles and tags for 2025" && \
		echo "" && \
		echo "--- Step 3: Create new-video branch ---" && \
		$(SUTURE) checkout main && \
		$(SUTURE) branch new-video && \
		$(SUTURE) checkout new-video && \
		mkdir -p videos/video-003 && \
		python3 -c "
import json
data = {
    'video_id': 'video-003',
    'title': 'Shaders for Beginners: Writing Your First GLSL Shader in Godot',
    'description': 'Learn the basics of shader programming with GLSL in Godot 4. We start from zero and build a water shader, an outline effect, and a dissolve effect.',
    'tags': ['shaders', 'glsl', 'godot', 'game development', 'beginner tutorial'],
    'category': '28',
    'visibility': 'public',
    'status': 'draft',
    'thumbnail': {'style': 'text_overlay', 'background_color': '#1a1a2e', 'text_color': '#ffffff', 'accent': '#53d8fb'}
}
with open('videos/video-003/metadata.json', 'w') as f: json.dump(data, f, indent=2)
" && \
		python3 -c "
import json
with open('playlists/gaming.json') as f: data = json.load(f)
data['videos'].append('video-003')
with open('playlists/gaming.json', 'w') as f: json.dump(data, f, indent=2)
" && \
		$(SUTURE) add videos/video-003/ playlists/gaming.json && \
		$(SUTURE) commit "Add video-003: Shaders for Beginners" && \
		echo "" && \
		echo "--- Step 4: Merge both branches into main ---" && \
		$(SUTURE) checkout main && \
		$(SUTURE) merge seo-update && \
		$(SUTURE) merge new-video && \
		echo "" && \
		echo "--- Step 5: Show log with graph ---" && \
		$(SUTURE) log --oneline --graph && \
		echo "" && \
		echo "--- Done: Channel metadata versioned with parallel branches ---"
	@rm -rf $(REPO)

clean:
	rm -rf $(REPO)
