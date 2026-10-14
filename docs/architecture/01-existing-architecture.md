# 01 — Architecture existante de WordCraft

> Statut : audit initial, 14 octobre 2026. Branche `claude/gallant-allen-2rb6nx`.
> Légende : **[code]** vérifié dans le code · **[doc]** indiqué par la documentation du dépôt · **[testé]** vérifié par un test exécuté · **[hypothèse]** à confirmer.

## 1. État du fork

| Élément | Constat |
|---|---|
| Dépôt de travail | `TheLazyGeekGuy/wordcraft` (fork public de `storytold/wordcraft`) |
| Remote | `origin` → `https://github.com/TheLazyGeekGuy/wordcraft` ; **pas de remote `upstream`** configuré dans ce clone |
| Branches | `main`, `claude/gallant-allen-2rb6nx` (branche de travail, identique à `main` au départ) |
| Modifications non validées | aucune au démarrage de l'audit (`git status` propre) |
| Dernier commit amont intégré | `eb54d88` « Mail merge: .xls recipients… » |
| Version | `0.7.0`, édition Rust 2024, MSRV 1.90 (`Cargo.toml`) |
| Licence du code | MIT OU Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`) |
| Répertoire `plan/` | absent (gitignoré en amont) — l'audit part de `ROADMAP.md`, `docs/gaps.md`, `docs/roadmap.md` |

Recommandation : ajouter le dépôt d'origine comme remote `upstream` (`git remote add upstream https://github.com/storytold/wordcraft`) pour récupérer les évolutions. Non fait ici : c'est une opération sur la configuration locale du porteur, à faire sur son poste.

## 2. Découpage en couches **[code]**

Le découpage est imposé par `cargo xtask layers` (`xtask/src/layers.rs:35-58`).

| Couche | Crate | Rôle | Intérêt pour le projet |
|---|---|---|---|
| L0 | `geom` | unités, mesures, régions (format de papier) | conservé tel quel |
| L1 | `doc` | modèle documentaire + primitives d'édition | **cœur conservé** |
| L1 | `fonts` | base de polices, shaping, substitution Word | conservé |
| L2 | `layout` | coupure de lignes, pagination, tableaux, hit-testing | conservé (pagination de l'aperçu et du PDF) |
| L2 | `docx` | lecture/écriture OOXML | devient **importateur** (l'écriture reste pour « Exporter en Word ») |
| L2 | `docbin` | lecture Word 97-2003 (`.doc`) | importateur |
| L2 | `formats` | TXT, **Markdown**, HTML, RTF, ODT, LaTeX | **lieu principal des modifications** |
| L3 | `render` | rastérisation vello_cpu | conservé |
| L3 | `pdf` | export PDF (krilla) | conservé |
| L3 | `proof` | orthographe / césure | conservé |
| L4 | `engine` | `Session`, undo, **registre de commandes**, dispatch I/O, catalogue Word | modifications ciblées (I/O, commandes de version) |
| L5 | `mcp` | serveur MCP | conservé |
| L6 | `ui-egui` | interface type Word (ruban, backstage) | ajouts (onglet Révision/Publication, panneau Historique) |
| apps | `wordcraft`, `wordcraft-cli`, `wordcraft-web` | binaires desktop, CLI, WASM | renommage ultérieur |

Autres crates : `metafile` (WMF/EMF), `zotero` (bibliographie), `control-key` (clé du canal de contrôle).

Principe structurant : **tout comportement utilisateur est une commande** (`CommandSpec` dans `crates/engine/src/cmd/*.rs`), appelable depuis le ruban, les raccourcis, la CLI, le canal de contrôle et MCP. Les nouvelles fonctions (enregistrer une version, synchroniser, comparer à une version) doivent suivre ce modèle.

## 3. Modèle documentaire en mémoire **[code]**

`crates/doc/src/lib.rs:623-650` — `Document` :

- `body: Blocks` avec `Blocks = Vec<Arc<Block>>` et `enum Block { Para(Paragraph), Table(Table) }` (lib.rs:78-98) ;
- `parts: BTreeMap<u32, Part>` : en-têtes, pieds, notes de bas de page, notes de fin, commentaires, zones de texte ;
- `styles`, `numbering`, `settings`, `core` (métadonnées), `sources` (bibliographie) ;
- `comments: BTreeMap<u32, Comment>` (auteur, initiales, date, parent pour les réponses, `resolved`) ;
- `revisions: Vec<Revision>` (suivi des modifications : insertion / suppression / format, auteur, date) ;
- `media: BTreeMap<String, Arc<Vec<u8>>>` : **octets des images**, référencés par clé ;
- `passthrough` : parties OOXML conservées telles quelles pour l'aller-retour DOCX.

`Paragraph` (para.rs:751) = `text: String` + `runs` (longueur + `CharProps`) + `objects: Vec<InlineObject>` ancrés sur U+FFFC. Les liens sont un attribut de run (`CharProps.link`), les images un `InlineObject::Image { media, w, h, alt, float, crop }`, les commentaires des paires `CommentStart/End`, les signets `BookmarkStart/End`, les notes `NoteRef`.

Conséquence : le modèle est **beaucoup plus riche que Markdown** ; il n'a pas à changer. La contrainte Markdown se place à la frontière de persistance (voir 03).

## 4. Modèle « flow » des formats texte **[code]**

`crates/formats/src/model.rs` définit un modèle intermédiaire réduit (`Flow` → `FBlock::Para | FBlock::Table`, `Inline::Text/Image/Anchor/Figure/Equation`, `Fmt`) avec `to_doc` / `from_doc`. **Tous** les formats texte (Markdown, HTML, RTF, ODT, LaTeX) passent par lui. C'est la « couche de transformation contrôlée » demandée par l'option B : elle existe déjà.

## 5. Entrées/sorties **[code]**

- `crates/engine/src/io.rs` : `OPEN_EXTS` (docx, docm, dotx, dotm, doc, dot, txt, **md, markdown**, html, rtf, odt, tex, xml, json), `SAVE_EXTS` (… **md** …, pdf, png). Dispatch par extension puis par signature (`open_by_content`).
- `crates/engine/src/io_ext.rs` : branche DOCX/DOC/XML vers `docx`/`docbin`, PDF/PNG, et le reste vers `wordcraft_formats::import_on/export_with`.
- Sauvegarde atomique (`io/safe_save.rs` : fichier temporaire, fsync, rename).
- **Format de sauvegarde par défaut : `.docx`** (`ui-egui/src/lib.rs:1547-1552`, `default_save_name`). Le `.md` est traité comme une copie (le document reste « modifié », `cmd/file.rs:286-289`).
- Auto-enregistrement : côté UI uniquement, toutes les 2,5 s, et seulement pour un format sans perte (`keeps_everything`) ; jamais dans le navigateur. **Pas de récupération après incident sur disque.**

## 6. Annuler / versions / révision **[code]**

- Undo par instantané complet (`Session.history`, `MAX_UNDO = 500`, `session.rs:283-292, 569`) : bon marché grâce aux `Arc`.
- **Versions** : `file.versions` (`cmd/tools.rs:396-412`) — instantanés nommés **en mémoire seulement** (`Session.versions`, max 50), perdus à la fermeture. `file.recover` liste ces versions.
- **Révision** (`cmd/review.rs`) : commentaires (nouveau, répondre, modifier, supprimer, résoudre), suivi des modifications (Ctrl+Maj+E), accepter/rejeter (un, tout), volet des modifications.
- **Comparer / Combiner** (`cmd/tools.rs:386-387`, `fn compare` :1018) : compare deux documents et produit un document avec révisions — **réutilisable tel quel pour « comparer deux versions Git »**.
- **Aucune intégration Git** et **aucune dépendance réseau HTTP** (`reqwest`, `ureq`, `gix`, `git2` absents du `Cargo.lock`). Seul `std::net::TcpStream` est utilisé (Zotero, MCP).

## 7. Interface **[code]**

- Ruban : `crates/engine/src/ribbon.rs:18-22` — onglets Fichier, Accueil, Insertion, Dessin, Création, Mise en page, Références, Publipostage, Révision, Affichage, Développeur, Zotero, Aide + onglets contextuels (Tableau, Image, Forme, Équation, Graphique, SmartArt).
- Onglet Révision : `ui-egui/src/ribbon.rs:1716-1790` (Vérification, Voix, Accessibilité, Langue, Commentaires, Suivi, Modifications, Comparer, Protéger, Encre).
- Backstage (menu Fichier) : `ui-egui/src/backstage.rs` (Accueil, Nouveau, Ouvrir, Informations, Enregistrer, Enregistrer sous, Imprimer, Exporter, Fermer, Options).
- Dialogues : `ui-egui/src/file_dialogs.rs` (filtres partagés), `apps/wordcraft/src/file_dialogs.rs` (rfd natif + explorateur intégré de secours).

## 8. PDF **[code]**

`crates/pdf/src/lib.rs` : `export(doc, &PdfOptions)` → mise en page puis krilla. Polices sous-ensemblées et embarquées avec mapping Unicode (texte sélectionnable), liens cliquables (URL et `#signet`), **signets PDF issus des titres** (`fn outline`, lib.rs:1441), PDF balisé (H1–H6, P, Figure avec texte alternatif). Fonctionne hors ligne et en WASM (compilé par `cargo xtask wasm`). Couvre toutes les exigences §7.3 du cahier des charges.

## 9. Web / WASM **[code]**

`apps/wordcraft-web` (Trunk + eframe/wgpu, repli WebGL2). Ouverture par sélecteur de fichier ou glisser-déposer, sauvegarde par `showSaveFilePicker` ou téléchargement Blob. Sur wasm : pas de système de fichiers, pas d'explorateur intégré, pas d'auto-enregistrement, pas de threads de rendu, pas de scan des polices système. `cargo xtask wasm` vérifie toutes les crates L0–L6 en `wasm32-unknown-unknown`.

## 10. Qualité et outillage **[code]**

- `cargo xtask ci` : fmt, clippy `-D warnings`, tests, assets, layers, wasm (`xtask/src/main.rs:156-205`).
- ~128 tests dans `formats`, ~556 dans `engine` (comptage `#[test]`).
- Règle « never crash » : pas de `unwrap/expect/panic` hors tests, `unsafe` interdit, entrées hostiles bornées.
- Règle d'actifs : aucun élément graphique Microsoft/Adobe ; chaque fichier d'actif a sa ligne dans `ATTRIBUTION.md`.

## 11. Identité et marques **[code]**

- Identifiant d'application `ai.storyteller.wordcraft` (`apps/wordcraft/src/main.rs:293`, `packaging/macos/Info.plist.in`, `packaging/linux/*`, flatpak).
- Windows : `packaging/windows/wordcraft.wxs` — fabricant « Learning Machines LLC » et `UpgradeCode` **propre à WordCraft** (à régénérer obligatoirement pour le fork).
- Chemins de configuration : `crates/control-key/src/lib.rs:7-18` (`~/.config/wordcraft`, `%APPDATA%\WordCraft`, `~/Library/Application Support/WordCraft`).
- Marques ArtCraft (`docs/brand/`, `NOTICE`, `LICENSE-brand.txt`, README:360-385) : **un fork modifié doit retirer ou remplacer** les logos et ne pas se présenter comme ArtCraft ; il peut se dire « basé sur WordCraft par l'équipe ArtCraft ». Le lien « À propos » pointe vers getartcraft.com et Discord ArtCraft (`ui-egui/src/dialogs.rs:2578-2586`).
- Icônes d'application (`assets/app-icon/`) : « placeholder », MIT/Apache — réutilisables mais à remplacer pour l'identité propre.
