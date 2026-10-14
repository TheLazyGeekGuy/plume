# 07 — Plan de migration et de développement

> Statut : proposition. Estimations en jours de développement pour une personne connaissant Rust, **[hypothèse]** à réviser après la phase 2.

## 1. Principes

1. **Rester proche de l'amont** : les crates gardent leurs noms internes `wordcraft-*`. Seuls les éléments visibles (nom, icônes, identifiants, chemins) changent. Cela permet de récupérer les améliorations de `storytold/wordcraft` (remote `upstream`) par simples fusions.
2. **Ajouter plutôt que modifier** : nouveau code dans des modules ou crates dédiés (`formats/markdown.rs` profil natif, crate `forge`, `VersionStore` dans `engine`).
3. **Chaque étape compile et passe `cargo xtask ci`** ; chaque comportement nouveau est une commande avec tests.
4. **Toute décision structurante a son ADR** (`docs/architecture/adr/`).

## 2. Phases

### Phase 0 — Audit ✅ (ce dossier)

### Phase 1 — Nouveau projet (identité) — 3 à 5 j

| Tâche | Fichiers |
|---|---|
| Choisir le nom (décision du porteur) | — |
| Ajouter le remote `upstream` | configuration locale |
| Retirer les marques ArtCraft (logo du README, `docs/brand/` hors dépôt amont, lien Discord/site dans « À propos ») | `README.md`, `docs/brand/`, `crates/ui-egui/src/dialogs.rs:2578` |
| Nouvel identifiant d'application (remplace `ai.storyteller.wordcraft`) | `apps/wordcraft/src/main.rs:293`, `default_apps.rs:35`, `packaging/macos/Info.plist.in`, `packaging/linux/*`, flatpak |
| **Nouveau `UpgradeCode` Windows** et fabricant | `packaging/windows/wordcraft.wxs:13-23` |
| Chemins de configuration propres (éviter de partager la configuration avec WordCraft installé) | `crates/control-key/src/lib.rs:7-18`, `crates/proof/src/hunspell.rs:60`, base IndexedDB `apps/wordcraft-web/src/web/font_store.rs:10` |
| Titres, métadonnées producteur PDF/DOCX, nom MCP | `ui-egui/src/lib.rs:1867`, `chrome.rs:350`, `pdf/src/lib.rs:366`, `docx/src/write/mod.rs:1237`, `mcp/src/server.rs:136` |
| Nouvelle icône **originale** dessinée en code ou CC0, avec ligne `ATTRIBUTION.md` | `assets/app-icon/`, `packaging/icons.sh` |
| `NOTICE` : mention « basé sur WordCraft par l'équipe ArtCraft », licences MIT/Apache conservées | `NOTICE`, `README.md` |

### Phase 2 — Markdown natif — 10 à 15 j (incrément 1 fait)

| Tâche | État |
|---|---|
| `export_native` zéro HTML, `assets/` à noms stables, `Report` | ✅ fait |
| `import_with` (images relatives) ; ouverture `.md` depuis le disque | ✅ fait |
| `io::save_markdown` ; un chemin `.md` sauvegarde en natif | ✅ fait |
| Test de bout en bout DOCX → MD → MD → PDF | ✅ fait |
| `.md` = format maître : nom par défaut à l'enregistrement, indicateur « modifié », auto-enregistrement | à faire (`ui-egui/src/lib.rs:1547, 2085`, `engine/src/cmd/file.rs:286`) |
| Rapport affiché après import et à l'enregistrement (bandeau non bloquant) | à faire |
| Front matter YAML (import + export) | à faire |
| Notes de bas de page `[^n]` | à faire |
| Commentaires ancrés `<!-- cN -->` (03 §6) | à faire |
| Liens internes vers les slugs de titres | à faire |
| Conversion PNG des images non web, graphiques/SmartArt rendus en image | à faire |
| Mode strict/souple des commandes (03 §5) | à faire |
| Thèmes visuels `gfm` / `vitepress` (feuilles de styles appliquées à l'import) | à faire |
| Profil de publication `publication/default.toml` (page, marges, polices) | à faire |

### Phase 3 — Import Word et PDF — 5 à 8 j

Corpus (09 §2), rapport d'import, conversion des ressources, Word 6/95 (repli facultatif), export PDF selon le profil de publication.

### Phase 4 — Stockage et Git — 15 à 25 j

`VersionStore` + `MemoryStore` (reprise de `file.versions`), `LocalGitStore` (`gix`), panneau Historique, « Enregistrer une version », « Afficher les modifications » via `review.compare`, restauration, journal de récupération, explorateur de dossier documentaire.

### Phase 5 — Forges et révision — 20 à 30 j

Crate `forge` (GitHub, GitLab, Forgejo), authentification (device flow / PKCE / jetons), trousseau, ouverture depuis une forge, synchronisation, conflits via `compare`, relecture (PR/MR), commentaires de forge ↔ commentaires Word, circuit de validation.

### Phase 6 — Distribution — 5 à 10 j

Paquets macOS/Windows/Linux/Web sous la nouvelle identité, documentation utilisateur en français, tests de compatibilité.

## 3. Stratégie de renommage

- **Ne pas** renommer les crates ni les modules (diffs illisibles, fusions amont impossibles).
- Centraliser le nom affiché dans une constante unique (par exemple `wordcraft_engine::APP_NAME`) et la référencer partout où « WordCraft » est visible — c'est une modification que l'amont pourrait même accepter.
- Renommer les binaires au niveau de l'empaquetage seulement (`[[bin]] name`), pas des packages Cargo.

## 4. Propositions de nom (décision du porteur)

Critères : pas de « Craft » (évite la confusion avec la marque ArtCraft), pas de référence à Word/Office, disponible comme nom de dépôt.

| Nom | Idée |
|---|---|
| **Plume** | écrire, simple, francophone |
| **Folio** | page, document |
| **Marque-page** / **Signet** | lien avec Markdown, lecture |
| **Calame** | instrument d'écriture |
| **Scriptorium** | atelier d'écriture collaboratif — écho au versionnement |

Vérifications à faire par le porteur : marques déposées, noms de domaine, dépôts existants.
