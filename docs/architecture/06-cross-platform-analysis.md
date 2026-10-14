# 06 — Analyse multiplateforme

> Statut : audit. Légende : **[code]** vérifié · **[hypothèse]** à confirmer.

## 1. Situation actuelle

| Plateforme | Binaire | Empaquetage existant | État |
|---|---|---|---|
| macOS (Apple Silicon, Intel) | `apps/wordcraft` (eframe) | `packaging/macos` (Info.plist, DMG signé) **[code]** | pris en charge |
| Windows x64 / ARM64 | idem | `packaging/windows/wordcraft.wxs` (MSI WiX) **[code]** | pris en charge |
| Linux x64 / ARM64 | idem | `packaging/linux` (.desktop, metainfo, MIME), Flatpak **[code]** | pris en charge |
| BSD | idem | README (Gentoo overlay mentionné, BSD dans CLAUDE.md) | annoncé |
| Web | `apps/wordcraft-web` (Trunk, eframe wgpu → WebGL2) | `packaging/web/package.sh` **[code]** | pris en charge |

Le code métier est **entièrement partagé** : toutes les crates L0–L6 compilent en `wasm32-unknown-unknown` (`cargo xtask wasm`, `xtask/src/main.rs:126-154`). Aucune réécriture n'est nécessaire.

## 2. Capacités par plateforme pour le projet

| Capacité | Desktop | Web | Commentaire |
|---|---|---|---|
| Édition WYSIWYG, ruban, styles | oui | oui | même code |
| Import DOCX / DOC / RTF / ODT / HTML | oui | oui | moteur pur Rust |
| Ouverture / sauvegarde `.md` | oui, avec `assets/` | **partiel** : un fichier à la fois via le sélecteur ; les images d'`assets/` ne sont pas accessibles | voir §3 |
| Export PDF | oui | oui (`print_pdf`, téléchargement) | krilla compilé en WASM |
| Polices | polices système + `CRAFT_FONTS_DIR` | polices embarquées + IndexedDB | thème visuel : choisir des polices libres incluses |
| Auto-enregistrement / récupération | à étendre au `.md` | **absent** (`autosave_block`) | IndexedDB/OPFS pour le journal |
| Git local | `gix` | **non** | |
| Forges (REST) | oui | oui, sous réserve de CORS et d'authentification (05 §4) | |
| Trousseau système | oui | non (session) | |
| Explorateur de documents | explorateur intégré existant (`ui-egui/src/file_browser.rs`) | non compilé en WASM | à remplacer par la vue « dépôt » via API |
| Threads de rendu | oui (rayon) | 0 (`render/src/lib.rs:24`) | 100 pages : à mesurer |

## 3. Obstacles propres au Web

1. **Système de fichiers.** Le navigateur ne donne qu'un fichier à la fois. Pour un `.md` + `assets/`, deux voies :
   - File System Access API (`showDirectoryPicker`, Chromium seulement) pour ouvrir un **dossier** ;
   - archive : proposer en téléchargement un `.zip` (`document.md` + `assets/`) — la crate `zip` est déjà une dépendance.
   Recommandation : dossier sous Chromium, `.zip` ailleurs, et **le dépôt distant comme mode principal du Web** (il n'a pas ce problème).
2. **Git.** Pas de processus, pas de sockets : uniquement les **API REST des forges** (05 §3.1). Pas de clone, pas de SSH.
3. **CORS.** GitHub et GitLab.com acceptent les appels d'API authentifiés depuis le navigateur **[hypothèse à tester]** ; Forgejo exige l'activation CORS sur l'instance ; les points d'entrée OAuth « device flow » de GitHub ne sont pas appelables depuis le navigateur → jeton personnel ou relais.
4. **Secrets.** Pas de trousseau : jeton en mémoire, durée de vie courte.
5. **Performance.** Rendu sans threads : mesurer l'édition d'un document de 100 pages (09 §6) avant de promettre la fluidité.
6. **Taille.** Le binaire WASM embarque tout le moteur ; ajouter un client Git en WASM est à proscrire (l'API REST suffit).

## 4. Abstractions à introduire

| Abstraction | Natif | Web |
|---|---|---|
| `HttpTransport` (requête → réponse) | client HTTP Rust (rustls) | `fetch` |
| `SecretStore` | trousseau système | mémoire de session |
| `DocumentStore` (lire/écrire `.md` + ressources) | système de fichiers | sélecteur de dossier / zip / forge |
| `VersionStore` | `MemoryStore`, `LocalGitStore`, `ForgeStore` | `MemoryStore`, `ForgeStore` |

Elles s'ajoutent au mécanisme `Services` déjà utilisé par l'interface pour les dialogues (`apps/wordcraft-web/src/web.rs:176`), sans dépendance réseau dans `engine`.

## 5. Priorités

| Plateforme | Priorité cahier des charges | Effort supplémentaire du projet |
|---|---|---|
| macOS Apple Silicon | P0 | faible (identité, signature, identifiant d'application) |
| macOS Intel, Windows x64, Linux x64 | P1 | faible |
| Web | P1 | **moyen à élevé** (stockage, forges, CORS, auth) |
| Windows ARM64, Linux ARM64 | P2 | faible (cross-compilation existante à vérifier) |
