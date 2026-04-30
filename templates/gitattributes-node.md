# Suture semantic merge drivers
# Install: curl -fsSL https://raw.githubusercontent.com/WyattAu/suture/main/scripts/install-merge-driver.sh | bash

package.json merge=json
package-lock.json merge=json
tsconfig.json merge=json
tsconfig.*.json merge=json
.eslintrc.json merge=json
.eslintrc.yml merge=yaml
.prettierrc merge=json
.prettierrc.json merge=json
.prettierrc.yml merge=yaml
.prettierrc.toml merge=toml
jest.config.json merge=json
jest.config.js -merge
babel.config.json merge=json
rollup.config.mjs -merge
vite.config.ts -merge
.next/merge.driver -merge
