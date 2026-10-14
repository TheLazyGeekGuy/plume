# 08 — Évaluation des risques

> Échelle : probabilité / impact Faible · Moyen · Élevé.

| # | Risque | Prob. | Impact | Mitigation |
|---|---|---|---|---|
| R1 | **Écart Word ↔ Markdown** : l'utilisateur applique couleurs, polices, fusions de cellules et les perd à l'enregistrement | Élevée | Élevé | Rapport d'adaptation (`Report`, implémenté), indicateur en barre d'état, mode strict, thème visuel qui montre dès l'édition le rendu final, export Word/PDF toujours disponible |
| R2 | **Perte silencieuse** de contenu (notes de bas de page, images non web, objets) | Moyenne | Élevé | Test d'invariant « tout ce qui est perdu est compté » ; corpus ; priorité aux notes `[^n]` et à la conversion PNG (04 §4) |
| R3 | **Dérive au fil des cycles** ouverture/enregistrement | Faible | Élevé | Idempotence testée (`native_markdown_is_html_free_and_stable`, `docx_to_native_markdown_round_trip`) ; à étendre en test de propriété (`proptest` est déjà une dépendance) |
| R4 | Tableaux complexes de documents administratifs (fusions, tableaux de mise en page) | Élevée | Moyen | Déroulage des tableaux de mise en page, report ; option « conserver ce tableau en image » à étudier |
| R5 | **Word 6/95 et antérieurs** non lus | Moyenne | Moyen | Message clair (existant) ; repli LibreOffice facultatif (desktop) |
| R6 | Authentification forges dans le navigateur (CORS, device flow) | Élevée | Moyen | Jetons personnels en premier, relais OAuth facultatif ; desktop d'abord |
| R7 | API Forgejo différente selon les versions d'instance | Moyenne | Moyen | Détection de version, tests contre Codeberg et une instance de test |
| R8 | **Conflits d'édition** simultanée sur un même fichier | Moyenne | Moyen | Contrôle par `sha` attendu, fusion via `review.compare`, verrou consultatif facultatif |
| R9 | Secrets mal stockés | Faible | Élevé | Trousseau système, aucune écriture de jeton sur disque, jamais exposé à MCP/contrôle |
| R10 | **Marques ArtCraft / identité** non retirées | Moyenne | Élevé (juridique) | Phase 1 dédiée, liste des fichiers en 07 §2 |
| R11 | Divergence avec l'amont rendant les fusions coûteuses | Moyenne | Moyen | Pas de renommage de crates, ajouts dans des modules séparés, fusions régulières |
| R12 | Performance Web (rendu sans threads) sur 100 pages | Moyenne | Moyen | Mesure en phase 3 ; la projection Markdown n'a lieu qu'à l'enregistrement |
| R13 | Commentaires HTML altérés par des outils tiers (formatteurs Markdown, éditeurs) | Moyenne | Faible | Format de bloc tolérant, ancres orphelines rattachées au paragraphe, rapport à l'ouverture |
| R14 | Contenu non fiable (dépôts, documents) : chemins `..`, images énormes | Moyenne | Élevé | Confinement `LocalImages` (existant, réutilisé), plafonds, règle « never crash » et test `hostile_params_never_panic` |
| R15 | Licence : intégration accidentelle de code GPL (Pandoc, LibreOffice) | Faible | Élevé | Outils externes en sous-processus uniquement, jamais liés ; règle *clean room* du dépôt |
| R16 | Scripts de conversion du porteur (`convertDoC`) non évalués | Certaine (à ce jour) | Faible | Accès au dépôt à accorder ; comparaison sur corpus (04 §7) |
