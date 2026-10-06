.PHONY: session-create session-index session-doctor session-import session-timeline session-timeline-refresh session-dashboard session-delivery-package session-delivery-package-suspect session-delivery-open session-workflow macos-image-plan macos-image-create macos-image-verify macos-image-map macos-image-resume macos-image-retry macos-image-retry-reverse macos-image-workflow dev-doctor macos-doctor codex-bootstrap codex-prompt codex-validation-quick codex-validation-quality codex-validation-full codex-validation-real-apfs codex-validation-image-workflow codex-validation-session-workflow codex-validation-tauri codex-artifacts quality-cli release-safety-gate quality-fixture quality-gate quality-evaluate fs-quality-fixture fs-quality-gate fs-quality-evaluate fragmented-quality-fixture fragmented-quality-gate fragmented-quality-evaluate apfs-quality-fixture apfs-quality-gate apfs-quality-evaluate real-apfs-quality-gate real-apfs-metadata-gate real-apfs-quality-evaluate quality-gate-all test

dev-doctor:
	python3 tools/dev/build_doctor.py

macos-doctor:
	python3 tools/macos/check_runtime_environment.py

# macOS + Codex handoff / validation helpers
codex-bootstrap:
	python3 tools/macos/bootstrap_codex_environment.py

codex-prompt:
	python3 tools/codex/print_codex_prompt.py

codex-validation-quick:
	python3 tools/codex/run_macos_validation.py --phase quick --continue-on-error

codex-validation-quality:
	python3 tools/codex/run_macos_validation.py --phase quality --continue-on-error

codex-validation-full:
	python3 tools/codex/run_macos_validation.py --phase full --continue-on-error

codex-validation-real-apfs:
	python3 tools/codex/run_macos_validation.py --phase real-apfs --continue-on-error

codex-validation-image-workflow:
	python3 tools/codex/run_macos_validation.py --phase image-workflow --continue-on-error

codex-validation-session-workflow:
	python3 tools/codex/run_macos_validation.py --phase session-workflow --continue-on-error

codex-validation-tauri:
	python3 tools/codex/run_macos_validation.py --phase tauri --continue-on-error

codex-artifacts:
	if [ -n "$(SESSION)" ]; then python3 tools/codex/collect_validation_artifacts.py --session "$(SESSION)"; else python3 tools/codex/collect_validation_artifacts.py; fi



# v2.0 Session/case folder workflow. Examples:
#   make session-create SOURCE=/dev/disk4 OUT=/Volumes/SAFE/Polik-Sessions MODE=apfs
#   make session-index SESSION=/Volumes/SAFE/Polik-Sessions/<case-id>
#   make session-doctor SESSION=/Volumes/SAFE/Polik-Sessions/<case-id>
#   make session-dashboard SESSION=/Volumes/SAFE/Polik-Sessions/<case-id>
#   make session-import SESSION=/Volumes/SAFE/Polik-Sessions/<case-id> KIND=auto PATHS="report.json report.html"
#   make session-timeline SESSION=/Volumes/SAFE/Polik-Sessions/<case-id> TITLE="完成掃描" STATUS=success SUMMARY="已輸出 scan report"
session-create:
	OUT_ROOT="$(OUT)"; if [ -z "$$OUT_ROOT" ]; then OUT_ROOT="$$HOME/Desktop/Polik-Recovery-Sessions"; fi; python3 tools/session/create_recovery_session.py --source "$(SOURCE)" --mode "$(MODE)" --out-root "$$OUT_ROOT"

session-index:
	python3 tools/session/create_recovery_session.py --session "$(SESSION)"

session-doctor:
	python3 tools/session/session_doctor.py --session "$(SESSION)"

session-import:
	python3 tools/session/import_session_artifacts.py --session "$(SESSION)" --kind "$${KIND:-auto}" $(PATHS)

session-timeline:
	python3 tools/session/session_timeline.py --session "$(SESSION)" --event-type "$(or $(EVENT),note)" --title "$(or $(TITLE),Session note)" --status "$(or $(STATUS),info)" --summary "$(SUMMARY)" --actor "$(or $(ACTOR),make session-timeline)"

session-timeline-refresh:
	python3 tools/session/session_timeline.py --session "$(SESSION)" --refresh

session-dashboard:
	python3 tools/session/session_dashboard.py --session "$(SESSION)"

session-workflow:
	@echo "1) make session-create SOURCE=/dev/diskX OUT=/Volumes/SAFE/Polik-Sessions MODE=apfs"
	@echo "2) 在 session/01_source_image 放 image manifest、ddrescue map report"
	@echo "3) 在 session/02_scan_reports 放 scan/VFS 報告"
	@echo "4) 還原到 session/03_recovery_output/，完整檔 recovered/，可疑檔 suspect/"
	@echo "5) make session-import SESSION=/Volumes/SAFE/Polik-Sessions/<case-id> KIND=auto PATHS='report.json report.html'"
	@echo "6) make session-timeline SESSION=/Volumes/SAFE/Polik-Sessions/<case-id> TITLE='完成掃描' STATUS=success SUMMARY='已匯入掃描報告'"
	@echo "7) make session-index SESSION=/Volumes/SAFE/Polik-Sessions/<case-id>"
	@echo "8) make session-dashboard SESSION=/Volumes/SAFE/Polik-Sessions/<case-id>"
	@echo "9) make session-doctor SESSION=/Volumes/SAFE/Polik-Sessions/<case-id>"


# Safe image-first recovery workflow. Usage examples:
#   make macos-image-plan SOURCE=/dev/disk4 OUT=/Volumes/SAFE/polik-images
#   make macos-image-create SOURCE=/dev/disk4 OUT=/Volumes/SAFE/polik-images
#   make macos-image-map MANIFEST=/Volumes/SAFE/polik-images/polik-image-disk4-YYYY.manifest.json
#   make macos-image-verify MANIFEST=/Volumes/SAFE/polik-images/polik-image-disk4-YYYY.manifest.json
#   make macos-image-retry MANIFEST=/Volumes/SAFE/polik-images/polik-image-disk4-YYYY.manifest.json
macos-image-plan:
	python3 tools/macos/create_source_image.py --source "$(SOURCE)" --out-dir "$(OUT)" --dry-run

macos-image-create:
	python3 tools/macos/create_source_image.py --source "$(SOURCE)" --out-dir "$(OUT)"

macos-image-verify:
	python3 tools/macos/verify_source_image.py --manifest "$(MANIFEST)"

macos-image-map:
	python3 tools/macos/ddrescue_map_report.py --manifest "$(MANIFEST)"

macos-image-resume:
	python3 tools/macos/create_source_image.py --resume-manifest "$(MANIFEST)" --method ddrescue

macos-image-retry:
	python3 tools/macos/create_source_image.py --resume-manifest "$(MANIFEST)" --method ddrescue --retry-bad --retries "$${RETRIES:-3}"

macos-image-retry-reverse:
	python3 tools/macos/create_source_image.py --resume-manifest "$(MANIFEST)" --method ddrescue --retry-bad --reverse --retries "$${RETRIES:-3}"

macos-image-workflow:
	@echo "1) make macos-image-plan SOURCE=/dev/diskX OUT=/Volumes/SAFE/polik-images"
	@echo "2) make macos-image-create SOURCE=/dev/diskX OUT=/Volumes/SAFE/polik-images"
	@echo "3) make macos-image-map MANIFEST=/Volumes/SAFE/polik-images/<name>.manifest.json"
	@echo "4) make macos-image-verify MANIFEST=/Volumes/SAFE/polik-images/<name>.manifest.json"
	@echo "5) 若 map 報告顯示 pending/bad：make macos-image-resume 或 macos-image-retry MANIFEST=..."
	@echo "6) 掃描 manifest 內 output_image 指向的 .img，不要反覆掃描原始磁碟。"

quality-fixture:
	python3 tools/quality/generate_fixture.py --out fixtures/quality/carver_baseline

quality-cli:
	cargo build --locked -p polik-recovery-cli

release-safety-gate:
	cargo build --release --locked -p polik-recovery-cli
	python3 tools/quality/check_release_fail_closed.py

quality-gate: quality-cli quality-fixture
	python3 tools/quality/run_quality_gate.py

quality-evaluate:
	python3 tools/quality/evaluate_recovery.py \
		--expected fixtures/quality/carver_baseline/expected_manifest.json \
		--manifest target/quality-gate/carver_output/carve_manifest.json \
		--out target/quality-gate/quality_report.json \
		--html target/quality-gate/quality_report.html

fs-quality-fixture:
	python3 tools/quality/generate_fs_fixtures.py --out fixtures/fs_quality

fs-quality-gate: quality-cli fs-quality-fixture
	python3 tools/quality/run_fs_quality_gate.py

fs-quality-evaluate:
	python3 tools/quality/evaluate_fs_recovery.py \
		--expected fixtures/fs_quality/expected_fs_manifest.json \
		--output-root target/fs-quality \
		--out target/fs-quality/fs_quality_report.json \
		--html target/fs-quality/fs_quality_report.html

fragmented-quality-fixture:
	python3 tools/quality/generate_fragmented_fixtures.py --out fixtures/fragmented_quality

fragmented-quality-gate: quality-cli fragmented-quality-fixture
	python3 tools/quality/run_fragmented_quality_gate.py

fragmented-quality-evaluate:
	python3 tools/quality/evaluate_fragmented_recovery.py \
		--expected fixtures/fragmented_quality/expected_fragmented_manifest.json \
		--output-root target/fragmented-quality \
		--out target/fragmented-quality/fragmented_quality_report.json \
		--html target/fragmented-quality/fragmented_quality_report.html

apfs-quality-fixture:
	python3 tools/quality/generate_apfs_fixtures.py --out fixtures/apfs_quality

apfs-quality-gate: quality-cli apfs-quality-fixture
	python3 tools/quality/run_apfs_quality_gate.py

apfs-quality-evaluate:
	python3 tools/quality/evaluate_apfs_recovery.py \
		--expected fixtures/apfs_quality/expected_apfs_manifest.json \
		--output-root target/apfs-quality \
		--out target/apfs-quality/apfs_quality_report.json \
		--html target/apfs-quality/apfs_quality_report.html


real-apfs-quality-gate:
	python3 tools/macos/run_real_apfs_quality_gate.py --cli target/release/polik-recovery

real-apfs-metadata-gate:
	python3 tools/macos/run_real_apfs_quality_gate.py --cli target/release/polik-recovery --metadata-only

real-apfs-quality-evaluate:
	python3 tools/quality/evaluate_real_apfs_recovery.py \
		--expected target/real-apfs-quality/fixture/expected_real_apfs_manifest.json \
		--output-root target/real-apfs-quality \
		--out target/real-apfs-quality/real_apfs_quality_report.json \
		--html target/real-apfs-quality/real_apfs_quality_report.html

quality-gate-all: quality-gate fs-quality-gate fragmented-quality-gate apfs-quality-gate release-safety-gate

test:
	cargo test

# v2.5 Session handoff / customer report workflow
session-report:
	python3 tools/session/session_handoff_report.py --session "$(SESSION)" --audience both

session-report-with-paths:
	python3 tools/session/session_handoff_report.py --session "$(SESSION)" --audience both --include-paths

session-customer-report:
	python3 tools/session/session_handoff_report.py --session "$(SESSION)" --audience customer

session-engineer-report:
	python3 tools/session/session_handoff_report.py --session "$(SESSION)" --audience engineer --include-paths

session-handoff-workflow:
	@echo "1) make session-dashboard SESSION=/Volumes/SAFE/Polik-Sessions/<case-id>"
	@echo "2) make session-report SESSION=/Volumes/SAFE/Polik-Sessions/<case-id>"
	@echo "3) 開啟 09_reports/customer_summary.html 給客戶/使用者看"
	@echo "4) 開啟 09_reports/engineer_handoff.html 給 Codex/工程師接手"
	@echo "5) make session-doctor SESSION=/Volumes/SAFE/Polik-Sessions/<case-id> 確認報告存在"

# v2.6 PDF export and manual sample audit workflow
session-audit:
	python3 tools/session/session_sample_audit.py --session "$(SESSION)" --sample-size "$${SAMPLE_SIZE:-5}"

session-audit-reset:
	python3 tools/session/session_sample_audit.py --session "$(SESSION)" --sample-size "$${SAMPLE_SIZE:-5}" --reset

session-audit-add:
	python3 tools/session/session_sample_audit.py --session "$(SESSION)" --add-file "$(FILE)" --result "$${RESULT:-opens_ok}" --checked-by "$${CHECKED_BY:-operator}" --note "$${NOTE:-}"

session-report-pdf:
	python3 tools/session/session_pdf_export.py --session "$(SESSION)" --audience both --ensure-audit

session-customer-pdf:
	python3 tools/session/session_pdf_export.py --session "$(SESSION)" --audience customer --ensure-audit

session-engineer-pdf:
	python3 tools/session/session_pdf_export.py --session "$(SESSION)" --audience engineer --ensure-audit

session-delivery-workflow:
	@echo "1) make session-report SESSION=/Volumes/SAFE/Polik-Sessions/<case-id>"
	@echo "2) make session-audit SESSION=/Volumes/SAFE/Polik-Sessions/<case-id> SAMPLE_SIZE=5"
	@echo "3) 人工開啟 sample_audit.html 內列出的樣本，逐筆用 make session-audit-add 記錄結果"
	@echo "4) make session-report-pdf SESSION=/Volumes/SAFE/Polik-Sessions/<case-id>"
	@echo "5) 開啟 09_reports/customer_summary.pdf 與 09_reports/engineer_handoff.pdf，確認字型與版面"
	@echo "6) make session-delivery-package SESSION=/Volumes/SAFE/Polik-Sessions/<case-id>，建立客戶安全交付 zip"

# v2.7 customer-safe delivery package workflow
session-delivery-package:
	python3 tools/session/session_delivery_package.py --session "$(SESSION)"

session-delivery-package-suspect:
	python3 tools/session/session_delivery_package.py --session "$(SESSION)" --include-all-suspect

session-delivery-package-report-only:
	python3 tools/session/session_delivery_package.py --session "$(SESSION)" --no-recovered

session-delivery-workflow-v27:
	@echo "1) make session-report SESSION=/Volumes/SAFE/Polik-Sessions/<case-id>"
	@echo "2) make session-audit SESSION=/Volumes/SAFE/Polik-Sessions/<case-id> SAMPLE_SIZE=5"
	@echo "3) make session-report-pdf SESSION=/Volumes/SAFE/Polik-Sessions/<case-id>"
	@echo "4) make session-delivery-package SESSION=/Volumes/SAFE/Polik-Sessions/<case-id>"
	@echo "5) 客戶交付 zip 會在 11_delivery/packages/，不會包含 raw image、map、工程 log 或工程版報告"
