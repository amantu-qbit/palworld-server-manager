# Data attribution

The reference catalogs in this directory (`items.json`, `active_skills.json`,
`passive_skills.json`, `elements.json`) are vendored from the English
localization data of **palworld-save-pal**:

- https://github.com/oMaN-Rod/palworld-save-pal
- Source path: `data/json/l10n/en/{items,active_skills,passive_skills,elements}.json`

palworld-save-pal itself derives its Palworld game-data catalogs from
**palworld-save-tools**:

- https://github.com/cheahjs/palworld-save-tools

These files are a static, read-only id -> display-name lookup (internal
Palworld static id, e.g. `"Wood"` or `"EPalWazaID::AirBlade"`, mapped to its
English `localized_name`). They are used here only to label decoded
inventory-item and pal-skill ids with human-readable names; no code was
copied from either project.

Files are vendored as-is (unmodified) at the time of writing. If Palworld
adds new items/skills, these catalogs may need to be refreshed from upstream.
