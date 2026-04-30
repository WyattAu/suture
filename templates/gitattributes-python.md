# Suture semantic merge drivers
pyproject.toml merge=toml
setup.cfg merge=ini
setup.py -merge
requirements.txt merge=txt
Pipfile merge=toml
Pipfile.lock -merge
.poetry.lock -merge
.mypy.ini merge=ini
.flake8 -merge
tox.ini merge=ini
pytest.ini merge=ini
.github/workflows/*.yml merge=yaml
