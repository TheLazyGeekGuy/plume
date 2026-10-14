# 04 — Stratégie de conversion

> Statut : recommandation. Légende : **[code]** vérifié · **[doc]** documentation publique des outils · **[hypothèse]** à vérifier sur corpus.

## 1. Recommandation

**Le moteur WordCraft est l'unique chaîne de conversion par défaut**, sur toutes les plateformes y compris le Web :

```text
.docx/.docm/.dotx ─ crates/docx ─┐
.doc (97-2003)   ─ crates/docbin ┤
.rtf / .odt / .html / .txt ──────┼─► Document ─► export_native ─► .md + assets/ + rapport
.md (natif)      ─ import_with ──┘
```

Aucune dépendance externe lourde n'est ajoutée. Un convertisseur externe n'est envisagé qu'en **repli facultatif, desktop uniquement**, pour les formats que WordCraft ne lit pas (Word 6/95, Word pour DOS, WordPerfect).

## 2. Comparatif

| Critère | WordCraft natif | Pandoc | LibreOffice headless | Bibliothèques Rust (`docx-rs`, `docx-rust`) |
|---|---|---|---|---|
| Formats lus | DOCX/DOCM/DOTX/DOTM, DOC 97-2003, Flat OPC, WordML 2003, RTF, ODT, HTML, TXT, MD, LaTeX **[code]** | DOCX, ODT, RTF, HTML, EPUB… ; **pas de `.doc`** **[doc]** | quasiment tout, y compris Word 6/95 et `.doc` anciens **[doc]** | DOCX seulement |
| Qualité attendue | élevée : moteur conçu pour la parité Word (styles, listes, tableaux, champs, notes, commentaires, révisions, images, WMF/EMF) **[code]** | bonne sur la structure, faible sur la mise en page **[doc]** | élevée **[doc]** | partielle (lecture surtout orientée génération) **[hypothèse]** |
| Pertes connues | voir 03 §4 (côté Markdown, pas côté import) | idem + numérotation, objets | passe par un format intermédiaire (DOCX/HTML) puis par notre chaîne | nombreuses |
| Dépendances | aucune nouvelle | binaire Haskell externe (~100 Mo) | suite bureautique complète (~500 Mo) | crate Rust |
| Licence | MIT/Apache **[code]** | **GPL-2.0+** : utilisable seulement comme **programme externe** invoqué, jamais lié ni copié (règle *clean room* du dépôt) | MPL-2.0 : programme externe | MIT |
| WebAssembly | **oui** (`cargo xtask wasm`) **[code]** | non (existe en WASM expérimental, très lourd) | non | oui |
| Effort d'intégration | **déjà intégré** | appel de processus + gestion d'erreurs | appel de processus | réécriture de la couche d'import |

Conclusion : seul LibreOffice apporte une valeur réelle (formats antérieurs à Word 97), et uniquement en repli. Pandoc n'apporte rien que WordCraft ne fasse déjà, et introduit une contrainte GPL. Les bibliothèques Rust sont en retrait par rapport au moteur existant.

## 3. Détail par format d'entrée

| Format | Composant | État | Actions |
|---|---|---|---|
| `.md`, `.markdown` | `formats::markdown::import_with` | natif, images relatives résolues **[code]** | front matter YAML (actuellement lu comme règle + texte), notes `[^n]`, listes de tâches, alertes `> [!NOTE]` |
| `.docx` & co | `crates/docx` | complet, y compris documents chiffrés **[code]** | aucune |
| `.doc` 97-2003 | `crates/docbin` | lecture seule complète (texte, styles, listes, tableaux, notes, images, dessins) **[code]** | aucune |
| `.doc` Word 6.0/95 | — | **refusé** avec un message clair (`docbin/src/fib.rs:117`) **[code]** | repli LibreOffice facultatif, ou nouvel importateur (spécification publique limitée) |
| `.rtf` | `formats::rtf` | lecture/écriture, révisions **[code]** | aucune |
| `.odt` | `formats::odt` | lecture/écriture **[code]** | aucune |
| `.html` | `formats::html` | lecture avec images locales **[code]** | aucune |
| `.txt` | `formats::txt` | détection d'encodage (UTF-8/16, Windows-1252, GBK…) **[code]** | aucune |
| `.pdf` | — | non éditable | consultation facultative, hors périmètre |

## 4. Conversion des ressources

| Cas | Traitement |
|---|---|
| PNG, JPEG, GIF, WebP, BMP | conservés tels quels, nom `img-<hash>.<ext>`, rechargés à l'ouverture (`model::sniff_image`) **[prototype]** |
| TIFF, SVG, EMF, WMF | **limite actuelle** : écrits dans `assets/` mais **non rechargés** à la réouverture (`sniff_image` ne les reconnaît pas) → l'image redevient son texte alternatif. **Priorité haute** : convertir en PNG à l'enregistrement |
| WMF / EMF (fréquents dans les `.doc`) | **à faire** : rendu vectoriel → PNG via `crates/metafile` + `crates/render` (ou SVG) |
| Graphiques, SmartArt, formes, WordArt | **à faire** : rendu PNG de l'objet (le moteur les dessine déjà à l'écran) ; texte alternatif conservé |
| Images rognées | **à faire** : appliquer le rognage avant l'écriture (sinon l'image complète réapparaît) |
| Objets OLE (Excel incorporé…) | image d'aperçu seulement, signalé |

## 5. Rapport d'importation

Le `Report` produit par `export_native` alimente le rapport demandé (§5.4 du cahier des charges). À l'ouverture d'un `.docx`/`.doc`, l'application calcule ce rapport **sans écrire de fichier** et l'affiche dans un bandeau non bloquant :

```text
Importation terminée — procedure.docx
Paragraphes : 156 · Titres : 12 · Tableaux : 8 · Images : 24 · Liens : 17
Éléments adaptés : 3   [Voir le détail]
```

Le détail liste les lignes `Report::lines()` (« 2 × cellules fusionnées écrites comme cellules vides »…). Les compteurs de structure (paragraphes, titres, liens) sont à ajouter au `Report` (**à faire**, trivial à partir du `Flow`).

## 6. Collage depuis Word

Le collage passe déjà par `crates/engine/src/cmd/paste.rs` (HTML et RTF du presse-papiers, images). Rien à changer : le contenu collé entre dans le `Document` riche, et c'est l'enregistrement qui applique le profil. Le mode « strict » (03 §5) pourrait proposer « Coller en conservant uniquement la structure » par défaut.

## 7. Scripts de conversion existants du porteur

Le porteur indique disposer de scripts de conversion DOC → Markdown (dépôt `TheLazyGeekGuy/convertDoC`, privé). **Cette session n'a pas obtenu l'accès à ce dépôt** : la comparaison reste à faire. Méthode proposée : passer le même corpus (09 §2) dans les deux chaînes et comparer les `.md` produits et les rapports ; reprendre dans WordCraft les règles de normalisation des scripts qui se révèlent meilleures (en réécrivant le code, sans copie si la licence des scripts l'interdit).
