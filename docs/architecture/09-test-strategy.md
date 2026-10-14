# 09 — Stratégie de test

## 1. Existant réutilisé

- `cargo xtask ci` : fmt, clippy `-D warnings`, tests de l'espace de travail, actifs, couches, WASM.
- `crates/engine/src/tests.rs::hostile_params_never_panic` : toutes les commandes avec paramètres absurdes (à garder vert pour les futures commandes `vcs.*`).
- `cargo xtask corpus [--download]` : corpus de documents existant de l'amont.
- `cargo xtask visual-diff` : comparaison visuelle de rendus.
- Tests aller-retour par format dans `crates/formats/src/tests.rs`.

## 2. Corpus documentaire

À constituer dans un dossier hors dépôt (les fichiers produits par Word ne doivent pas être commités — règle *clean room*) ; seuls des documents **synthétiques** générés par le code entrent dans le dépôt.

| Cas | Source |
|---|---|
| Document simple | généré (`sample::sample_document`) |
| 100 pages | généré (répétition contrôlée) |
| Nombreuses images (PNG, JPEG, EMF, TIFF) | généré |
| Tableaux simples, fusionnés, imbriqués, de mise en page | généré |
| Listes multiniveaux | généré |
| Table des matières, notes, liens internes/externes | généré |
| Word 97-2003 `.doc` | corpus `xtask corpus` |
| DOCX récents réels | documents du porteur (hors dépôt) |
| Éléments non représentables (couleurs, WordArt, sections, en-têtes) | généré |

## 3. Tests de conservation (§16.2)

Implémentés dans cette branche :

| Test | Vérifie |
|---|---|
| `formats::tests::native_markdown_is_html_free_and_stable` | zéro HTML ; images en `assets/` ; `export ∘ import ∘ export` identique sur 3 cycles |
| `formats::tests::native_markdown_assets_are_content_named` | nom d'image = contenu, dédoublonnage, pas de fuite d'état entre imports |
| `formats::tests::native_markdown_reports_adaptations` | `<br>` et `<a id>` supprimés et signalés |
| `formats::tests::html_lines_finds_tags_but_not_comments_code_or_autolinks` | le détecteur de HTML lui-même |
| `engine::tests::docx_to_native_markdown_round_trip` | jalon complet : DOCX → `.md` + `assets/` → réouverture → réenregistrement identique → édition par commande → réouverture → PDF |

À ajouter :

1. **Propriété** (`proptest`) : pour tout document généré, `export_native(import_with(export_native(d))) == export_native(d)` et `html_lines == []`.
2. **Invariant de non-perte silencieuse** : tout texte visible du document d'origine se retrouve dans le `.md`, sauf ce que le `Report` compte (comparaison des textes normalisés).
3. **Séparation syntaxe / sémantique** : comparer deux `.md` par leur `Flow` (structure, textes, liens, images) plutôt que par octets, pour distinguer les différences purement syntaxiques des pertes réelles.
4. Corpus complet en tâche planifiée (pas à chaque commit) : rapport agrégé des adaptations par document.

## 4. Tests de sécurité

- Images : chemins `..`, absolus, URL, liens symboliques, fichiers énormes, milliers de références (limites `LocalImages` + `MAX_IMAGE_REFS`).
- Markdown hostile : imbrications profondes, crochets non fermés (budgets existants du parseur), commentaires HTML non fermés.
- Réponses de forge malformées ou gigantesques (crate `forge`, phase 5).

## 5. Tests Git / forges (phases 4-5)

- `LocalGitStore` : dépôts temporaires créés par le test ; historique, restauration, version avec images.
- `ForgeStore` : transport HTTP simulé (réponses JSON enregistrées à partir de la documentation publique), pas d'appel réseau dans `cargo test`.
- Scénarios de conflit : deux sessions modifient le même paragraphe / des paragraphes différents.
- Tests d'intégration réels, manuels ou planifiés, contre un dépôt de test sur chaque forge (jamais avec les identifiants du porteur dans le dépôt).

## 6. Performance

Mesures (ajoutées à `cargo xtask stats` ou à un banc dédié) : ouverture et enregistrement `.md` d'un document de 100 pages, latence de frappe, temps d'export PDF, en natif et en WASM.

## 7. Ergonomie (§16.3)

Scénarios exécutés par des utilisateurs ne connaissant ni Markdown ni Git, chronométrés et comparés à Word : ouvrir un DOCX, modifier un titre, insérer une image, modifier un tableau, enregistrer, fermer/rouvrir, exporter en PDF, enregistrer une version, revenir à une version précédente, lire les commentaires d'un collègue. Les mêmes parcours sont automatisés par le canal de contrôle (`docs/control-protocol.md`) et des captures `ui_shot` sans voler le focus.
