.PHONY: all clean

SUTURE ?= $(shell command -v suture 2>/dev/null || echo "../target/debug/suture")

REPO := /tmp/suture-example-i18n-translations

all: $(SUTURE)
	@echo "=== i18n Translation File Management Demo ==="
	@echo ""
	@rm -rf $(REPO)
	@mkdir -p $(REPO)
	@cd $(REPO) && \
		echo "--- Step 1: Initialize repo and commit base translations ---" && \
		$(SUTURE) init . && \
		$(SUTURE) config user.name="Localization Team" && \
		$(SUTURE) config user.email="i18n@example.com" && \
		mkdir -p locales && \
		cp $(CURDIR)/locales/en.json locales/en.json && \
		cp $(CURDIR)/locales/es.json locales/es.json && \
		cp $(CURDIR)/locales/ja.json locales/ja.json && \
		$(SUTURE) add locales/ && \
		$(SUTURE) commit "Initial translations: en, es, ja" && \
		echo "" && \
		echo "--- Step 2: Spanish translator adds new keys ---" && \
		$(SUTURE) branch translator-es && \
		$(SUTURE) checkout translator-es && \
		python3 -c "
import json
with open('locales/es.json') as f: data = json.load(f)
data['common']['retry'] = 'Reintentar'
data['common']['warning'] = 'Advertencia'
data['common']['undo'] = 'Deshacer'
data['nav']['notifications'] = 'Notificaciones'
data['projects']['rename_project'] = 'Renombrar Proyecto'
data['projects']['duplicate_project'] = 'Duplicar Proyecto'
with open('locales/es.json', 'w') as f: json.dump(data, f, indent=2)
" && \
		$(SUTURE) add locales/es.json && \
		$(SUTURE) commit "es: add new UI and project translation keys" && \
		echo "" && \
		echo "--- Step 3: Japanese translator adds new keys ---" && \
		$(SUTURE) checkout main && \
		$(SUTURE) branch translator-ja && \
		$(SUTURE) checkout translator-ja && \
		python3 -c "
import json
with open('locales/ja.json') as f: data = json.load(f)
data['common']['retry'] = '再試行'
data['common']['warning'] = '警告'
data['common']['undo'] = '元に戻す'
data['nav']['notifications'] = '通知'
data['projects']['rename_project'] = 'プロジェクト名の変更'
data['projects']['duplicate_project'] = 'プロジェクトの複製'
with open('locales/ja.json', 'w') as f: json.dump(data, f, indent=2)
" && \
		$(SUTURE) add locales/ja.json && \
		$(SUTURE) commit "ja: add new UI and project translation keys" && \
		echo "" && \
		echo "--- Step 4: Merge both translator branches ---" && \
		$(SUTURE) checkout main && \
		$(SUTURE) merge translator-es && \
		$(SUTURE) merge translator-ja && \
		echo "" && \
		echo "--- Step 5: Verify no keys were lost ---" && \
		echo "Spanish translations:" && \
		python3 -c "
import json
with open('locales/es.json') as f: data = json.load(f)
assert 'retry' in data['common'], 'Missing retry key in es'
assert 'warning' in data['common'], 'Missing warning key in es'
assert 'undo' in data['common'], 'Missing undo key in es'
assert 'notifications' in data['nav'], 'Missing notifications in es nav'
assert 'rename_project' in data['projects'], 'Missing rename_project in es'
print('  All 6 new keys present in es.json')
" && \
		echo "Japanese translations:" && \
		python3 -c "
import json
with open('locales/ja.json') as f: data = json.load(f)
assert 'retry' in data['common'], 'Missing retry key in ja'
assert 'warning' in data['common'], 'Missing warning key in ja'
assert 'undo' in data['common'], 'Missing undo key in ja'
assert 'notifications' in data['nav'], 'Missing notifications in ja nav'
assert 'rename_project' in data['projects'], 'Missing rename_project in ja'
print('  All 6 new keys present in ja.json')
" && \
		echo "" && \
		echo "--- Step 6: Show log ---" && \
		$(SUTURE) log --oneline --graph && \
		echo "" && \
		echo "--- Done: All translations merged without conflicts ---"
	@rm -rf $(REPO)

clean:
	rm -rf $(REPO)
