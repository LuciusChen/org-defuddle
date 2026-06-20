;;; org-defuddle-test.el --- Tests for org-defuddle  -*- lexical-binding: t; -*-

;; SPDX-License-Identifier: MIT

;;; Code:

(require 'ert)
(require 'cl-lib)

(defvar org-defuddle--module-loaded)
(defvar org-defuddle--module-version)
(declare-function org-defuddle--default-module-file "org-defuddle")
(declare-function org-defuddle--module-download-url "org-defuddle")
(declare-function org-defuddle--module-release-asset "org-defuddle")
(declare-function org-defuddle-download-module "org-defuddle" (&optional path))
(declare-function org-defuddle-html-to-org "org-defuddle" (html &optional url options))
(declare-function org-defuddle-load-module "org-defuddle" (&optional offer-download))

(defconst org-defuddle-test--root
  (file-name-directory
   (directory-file-name
    (file-name-directory (or load-file-name buffer-file-name))))
  "Repository root used by the org-defuddle tests.")

(load (expand-file-name "org-defuddle.el" org-defuddle-test--root) nil nil t)

(ert-deftest org-defuddle-test-release-assets-match-supported-platforms ()
  (dolist (case '((darwin "aarch64-apple-darwin" ".dylib"
                          "liborg-defuddle-aarch64-apple-darwin.dylib")
                  (darwin "x86_64-apple-darwin" ".dylib"
                          "liborg-defuddle-x86_64-apple-darwin.dylib")
                  (gnu/linux "x86_64-unknown-linux-gnu" ".so"
                             "liborg-defuddle-x86_64-unknown-linux-gnu.so")
                  (gnu/linux "aarch64-unknown-linux-gnu" ".so"
                             "liborg-defuddle-aarch64-unknown-linux-gnu.so")
                  (windows-nt "x86_64-pc-windows-msvc" ".dll"
                              "liborg-defuddle-x86_64-pc-windows-msvc.dll")))
    (pcase-let ((`(,platform ,configuration ,suffix ,asset) case))
      (let ((system-type platform)
            (system-configuration configuration)
            (module-file-suffix suffix))
        (should (equal (org-defuddle--module-release-asset) asset))))))

(ert-deftest org-defuddle-test-download-url-is-version-pinned ()
  (let ((system-type 'darwin)
        (system-configuration "aarch64-apple-darwin")
        (module-file-suffix ".dylib"))
    (should
     (equal
      (org-defuddle--module-download-url)
      (concat "https://github.com/LuciusChen/org-defuddle/releases/download/"
              org-defuddle--module-version
              "/liborg-defuddle-aarch64-apple-darwin.dylib")))))

(ert-deftest org-defuddle-test-installed-module-path-is-version-pinned ()
  (let ((user-emacs-directory "/tmp/org-defuddle-emacs/")
        (module-file-suffix ".dylib"))
    (should
     (equal
      (org-defuddle--installed-module-file)
      (concat user-emacs-directory "modules/liborg_defuddle_module-"
              org-defuddle--module-version ".dylib")))))

(ert-deftest org-defuddle-test-download-installs-and-loads-default-path ()
  (let* ((directory (make-temp-file "org-defuddle-module-" t))
         (org-defuddle-module-file
          (expand-file-name (concat "liborg_defuddle_module" module-file-suffix)
                            directory))
         (org-defuddle--module-loaded nil)
         requested-urls
         loaded-file)
    (unwind-protect
        (cl-letf (((symbol-function 'url-copy-file)
                   (lambda (url path &optional _ok-if-already-exists)
                     (push url requested-urls)
                     (with-temp-file path
                       (if (string-suffix-p "/SHA256SUMS" url)
                           (insert (format "%s  %s\n"
                                           (secure-hash 'sha256 "module")
                                           (org-defuddle--module-release-asset)))
                         (insert "module")))))
                  ((symbol-function 'org-defuddle--load-module-file)
                   (lambda (path)
                     (setq loaded-file path
                           org-defuddle--module-loaded t))))
          (org-defuddle-download-module)
          (should (member (org-defuddle--module-download-url) requested-urls))
          (should (member (org-defuddle--module-checksums-url) requested-urls))
          (should (equal loaded-file org-defuddle-module-file))
          (should (file-exists-p org-defuddle-module-file)))
      (delete-directory directory t))))

(ert-deftest org-defuddle-test-download-rejects-checksum-mismatch ()
  (let* ((directory (make-temp-file "org-defuddle-module-" t))
         (path (expand-file-name (concat "liborg_defuddle_module"
                                         module-file-suffix)
                                 directory))
         loaded-file)
    (unwind-protect
        (cl-letf (((symbol-function 'url-copy-file)
                   (lambda (url target &optional _ok-if-already-exists)
                     (with-temp-file target
                       (if (string-suffix-p "/SHA256SUMS" url)
                           (insert (format "%s  %s\n"
                                           (make-string 64 ?0)
                                           (org-defuddle--module-release-asset)))
                         (insert "module")))))
                  ((symbol-function 'org-defuddle--load-module-file)
                   (lambda (module-file) (setq loaded-file module-file))))
          (should-error (org-defuddle-download-module path))
          (should-not loaded-file)
          (should-not (file-exists-p path)))
      (delete-directory directory t))))

(ert-deftest org-defuddle-test-interactive-load-offers-release-download ()
  (let ((org-defuddle--module-loaded nil)
        offered
        downloaded)
    (cl-letf (((symbol-function 'org-defuddle--existing-module-file)
               (lambda () nil))
              ((symbol-function 'yes-or-no-p)
               (lambda (prompt)
                 (setq offered prompt)
                 t))
              ((symbol-function 'org-defuddle-download-module)
               (lambda (&optional _path)
                 (setq downloaded t
                       org-defuddle--module-loaded t))))
      (org-defuddle-load-module t)
      (should (string-match-p "download pre-built release" offered))
      (should downloaded))))

(ert-deftest org-defuddle-test-noninteractive-load-does-not-download ()
  (let ((org-defuddle--module-loaded nil))
    (cl-letf (((symbol-function 'org-defuddle--existing-module-file)
               (lambda () nil)))
      (should-error (org-defuddle-load-module) :type 'user-error))))

(ert-deftest org-defuddle-test-output-backend-defaults-to-temporary-buffer ()
  (let ((org-defuddle-output-backend 'buffer)
        (org "* Captured title\n\nCaptured body."))
    (when-let* ((buffer (get-buffer "*org-defuddle*")))
      (kill-buffer buffer))
    (unwind-protect
        (progn
          (org-defuddle--insert-org-buffer org)
          (should (get-buffer "*org-defuddle*"))
          (with-current-buffer "*org-defuddle*"
            (should (equal (buffer-string) org))))
      (when-let* ((buffer (get-buffer "*org-defuddle*")))
        (kill-buffer buffer)))))

(ert-deftest org-defuddle-test-denote-backend-creates-org-note ()
  (let* ((directory (make-temp-file "org-defuddle-denote-" t))
         (path (expand-file-name "note.org" directory))
         (org-defuddle-output-backend 'denote)
         (org-defuddle-note-keywords '("web" "reference"))
         (org-defuddle-denote-directory directory)
         (org "* Captured title\n:PROPERTIES:\n:URL: https://example.com\n:END:\n\nCaptured body.")
         (real-require (symbol-function 'require))
         denote-args
         displayed)
    (unwind-protect
        (cl-letf (((symbol-function 'require)
                   (lambda (feature &rest args)
                     (if (eq feature 'denote)
                         t
                       (apply real-require feature args))))
                  ((symbol-function 'denote)
                   (lambda (&rest args)
                     (setq denote-args args)
                     (with-temp-file path
                       (insert "#+title: Captured title\n"))
                     path))
                  ((symbol-function 'pop-to-buffer)
                   (lambda (buffer &rest _args)
                     (setq displayed buffer))))
          (org-defuddle--insert-org-buffer org)
          (should (equal (car denote-args) "Captured title"))
          (should (equal (cadr denote-args) '("web" "reference")))
          (should (eq (nth 2 denote-args) 'org))
          (should (equal (nth 3 denote-args) directory))
          (should (buffer-live-p displayed))
          (with-temp-buffer
            (insert-file-contents path)
            (should (search-forward "* Captured title" nil t))
            (should (search-forward "Captured body." nil t))))
      (when-let* ((buffer (find-buffer-visiting path)))
        (kill-buffer buffer))
      (delete-directory directory t))))

(ert-deftest org-defuddle-test-org-roam-backend-captures-org-note ()
  (let ((org-defuddle-output-backend 'org-roam)
        (org-defuddle-note-keywords '("web" "reference"))
        (org "* Captured title\n\nProgress is 100%.")
        (real-require (symbol-function 'require))
        (captured (generate-new-buffer "org-defuddle-test-roam-note"))
        capture-args
        node-args)
    (unwind-protect
        (cl-letf (((symbol-function 'require)
                   (lambda (feature &rest args)
                     (if (eq feature 'org-roam-capture)
                         t
                       (apply real-require feature args))))
                  ((symbol-function 'org-roam-node-create)
                   (lambda (&rest args)
                     (setq node-args args)
                     'node))
                  ((symbol-function 'org-roam-capture-)
                   (lambda (&rest args)
                     (setq capture-args args)
                     ;; Simulate :jump-to-captured by making the new note
                     ;; buffer current, so the post-capture insert lands
                     ;; there.
                     (set-buffer captured))))
          (org-defuddle--insert-org-buffer org)
          (should (equal node-args '(:title "Captured title")))
          (should (eq (plist-get capture-args :node) 'node))
          (should (equal (plist-get (plist-get capture-args :info)
                                    :org-defuddle-tags)
                         ":web:reference:"))
          ;; The capture template carries only the trusted header; the
          ;; body function is empty and the article is inserted verbatim
          ;; afterwards, so % survives unescaped.
          (let* ((entry (car (plist-get capture-args :templates)))
                 (body (nth 3 entry))
                 (content-function (cadr body)))
            (should (eq (car body) 'function))
            (should (equal (funcall content-function) "")))
          (with-current-buffer captured
            (should (equal (buffer-string)
                           "* Captured title\n\nProgress is 100%."))))
      (when (buffer-live-p captured)
        (kill-buffer captured)))))

(ert-deftest org-defuddle-test-org-roam-backend-keeps-template-syntax-literal-in-body ()
  "Article bodies containing ${...} must not reach org-roam's template engine.

This regression covers the failure where the extracted body was routed
through the capture template and `org-roam-format-template' interpreted
every ${...} as a placeholder: unknown keys triggered
`read-from-minibuffer', and names bound to functions were called with
the capture node as their argument."
  (let ((org-defuddle-output-backend 'org-roam)
        (org-defuddle-note-keywords nil)
        (org (concat "* Unable to run EAF on NixOS under Sway\n\n"
                     "#+begin_src\n"
                     "QT_QPA_PLATFORM_PLUGIN_PATH = \"${pkgs.qt6.qtwayland.outPath}\";\n"
                     "#+end_src\n"))
        (real-require (symbol-function 'require))
        (captured (generate-new-buffer "org-defuddle-test-roam-template"))
        prompted
        capture-args)
    (unwind-protect
        (cl-letf (((symbol-function 'require)
                   (lambda (feature &rest args)
                     (if (eq feature 'org-roam-capture)
                         t
                       (apply real-require feature args))))
                  ((symbol-function 'org-roam-node-create)
                   (lambda (&rest args) 'node))
                  ((symbol-function 'read-from-minibuffer)
                   (lambda (&rest _args)
                     (setq prompted t)
                     ""))
                  ((symbol-function 'org-roam-capture-)
                   (lambda (&rest args)
                     (setq capture-args args)
                     ;; Faithfully mirror `org-roam-format-template':
                     ;; it processes the template body returned by the
                     ;; capture entry's function and treats every ${...}
                     ;; as a placeholder that would prompt (or call a
                     ;; fboundp symbol named inside).
                     (let* ((entry (car (plist-get args :templates)))
                            (body (nth 3 entry))
                            (body-content (funcall (cadr body))))
                       (when (string-match-p (regexp-quote "${") body-content)
                         (read-from-minibuffer "would-prompt: ")))
                     (set-buffer captured))))
          (org-defuddle--insert-org-buffer org)
          ;; No ${...} reached the template body, so no prompt and no
          ;; function invocation.
          (should-not prompted)
          ;; The body landed in the note buffer verbatim, ${...} intact.
          (let* ((entry (car (plist-get capture-args :templates)))
                 (body (nth 3 entry))
                 (content-function (cadr body)))
            (should (equal (funcall content-function) "")))
          (with-current-buffer captured
            (should (equal (buffer-string) org))
            (goto-char (point-min))
            (should (search-forward "${pkgs.qt6.qtwayland.outPath}" nil t))))
      (when (buffer-live-p captured)
        (kill-buffer captured)))))

(ert-deftest org-defuddle-test-youtube-empty-caption-tries-next-client ()
  (let (inserted retry-args)
    (cl-letf (((symbol-function 'org-defuddle-parse-youtube-json)
               (lambda (&rest _args)
                 '(:org "description only"
                   :variables (:language "zh-Hant-HK"))))
              ((symbol-function 'org-defuddle--insert-org-buffer)
               (lambda (org) (setq inserted org)))
              ((symbol-function 'org-defuddle--youtube-fetch-player-chain)
               (lambda (&rest args) (setq retry-args args))))
      (org-defuddle--youtube-insert-result
       "player"
       ""
       "chapters"
       "https://www.youtube.com/watch?v=XRPZy_wJ_n8"
       "zh-Hant-HK"
       "zh-CN"
       '(:language "zh-CN")
       "XRPZy_wJ_n8"
       '((:name "ANDROID")))
      (should-not inserted)
      (should
       (equal retry-args
              '("https://www.youtube.com/watch?v=XRPZy_wJ_n8"
                "XRPZy_wJ_n8"
                "zh-CN"
                (:language "zh-CN")
                ((:name "ANDROID"))))))))

(ert-deftest org-defuddle-test-youtube-caption-inserts-only-with-transcript ()
  (let (inserted retried)
    (cl-letf (((symbol-function 'org-defuddle-parse-youtube-json)
               (lambda (&rest _args)
                 '(:org "* Video\n\n** Transcript\n\n字幕正文"
                   :variables (:transcript "字幕正文"
                               :language "zh-CN"))))
              ((symbol-function 'org-defuddle--insert-org-buffer)
               (lambda (org) (setq inserted org)))
              ((symbol-function 'org-defuddle--youtube-fetch-player-chain)
               (lambda (&rest _args) (setq retried t))))
      (org-defuddle--youtube-insert-result
       "player" "caption" "chapters" "https://youtu.be/video"
       "zh-CN" "zh-CN" '(:language "zh-CN") "video" nil)
      (should (equal inserted "* Video\n\n** Transcript\n\n字幕正文"))
      (should-not retried))))

(ert-deftest org-defuddle-test-youtube-inline-empty-caption-uses-fetched-page ()
  (let (inserted fallback-args)
    (cl-letf (((symbol-function 'org-defuddle-parse-youtube-json)
               (lambda (&rest _args)
                 '(:org "description only" :variables nil)))
              ((symbol-function 'org-defuddle--insert-org-buffer)
               (lambda (org) (setq inserted org)))
              ((symbol-function 'org-defuddle--youtube-insert-html-fallback)
               (lambda (&rest args) (setq fallback-args args))))
      (org-defuddle--youtube-insert-inline-result
       "<html>page</html>" "player" "" "{}"
       "https://www.youtube.com/watch?v=video" "zh" '(:language "zh-CN"))
      (should-not inserted)
      (should
       (equal fallback-args
              '("<html>page</html>"
                "https://www.youtube.com/watch?v=video"
                (:language "zh-CN")))))))

(ert-deftest org-defuddle-test-youtube-inline-fallback-uses-current-caption-info ()
  (let (chapter-args static-fallback)
    (cl-letf (((symbol-function 'org-defuddle--retrieve-body)
               (lambda (_url callback &rest _args)
                 (funcall callback "<html>page</html>")))
              ((symbol-function 'org-defuddle-youtube-inline-caption-info)
               (lambda (&rest _args)
                 '(:player_json "player"
                   :caption_url "https://www.youtube.com/api/timedtext?v=video"
                   :language "zh")))
              ((symbol-function 'org-defuddle--youtube-fetch-inline-chapters)
               (lambda (&rest args) (setq chapter-args args)))
              ((symbol-function 'org-defuddle--youtube-insert-html-fallback)
               (lambda (&rest _args) (setq static-fallback t))))
      (org-defuddle--youtube-fetch-inline-fallback
       "https://www.youtube.com/watch?v=video"
       "video"
       "zh-CN"
       '(:language "zh-CN"))
      (should-not static-fallback)
      (should
       (equal chapter-args
              '("<html>page</html>"
                "https://www.youtube.com/watch?v=video"
                "video"
                "zh-CN"
                (:language "zh-CN")
                (:player_json "player"
                 :caption_url "https://www.youtube.com/api/timedtext?v=video"
                 :language "zh")))))))

(ert-deftest org-defuddle-test-youtube-inline-info-is-optional-for-old-modules ()
  (let* ((function 'org-defuddle-module-youtube-inline-caption-info)
         (had-function (fboundp function))
         (saved-function (and had-function (symbol-function function)))
         (org-defuddle--module-loaded t))
    (unwind-protect
        (progn
          (when had-function
            (fmakunbound function))
          (cl-letf (((symbol-function 'org-defuddle-load-module)
                     (lambda (&optional _offer-download) t)))
            (should-not
             (org-defuddle-youtube-inline-caption-info
              "<html></html>"
              "https://www.youtube.com/watch?v=video"
              "zh-CN"))))
      (when had-function
        (fset function saved-function)))))

(ert-deftest org-defuddle-test-real-module-call ()
  (let ((org-defuddle-module-file (org-defuddle--default-module-file))
        (org-defuddle--module-loaded nil))
    (unless (file-exists-p org-defuddle-module-file)
      (ert-skip (format "Missing built module at %s" org-defuddle-module-file)))
    (org-defuddle-load-module)
    (should
     (string-match-p
      "Rust dynamic module extraction works"
      (org-defuddle-html-to-org
       (concat "<article><h1>Module Test</h1>"
               "<p>Rust dynamic module extraction works through Emacs.</p>"
               "</article>")
       "https://example.com/module-test")))
    (let ((caption-info
           (org-defuddle-youtube-inline-caption-info
            (concat
             "<script>var ytInitialPlayerResponse = {"
             "\"videoDetails\":{\"videoId\":\"module123\"},"
             "\"captions\":{\"playerCaptionsTracklistRenderer\":{"
             "\"captionTracks\":[{\"baseUrl\":"
             "\"https://www.youtube.com/api/timedtext?v=module123&lang=en\","
             "\"languageCode\":\"en\"}]}}};</script>")
            "https://www.youtube.com/watch?v=module123"
            "en")))
      (should (equal (plist-get caption-info :language) "en"))
      (should (string-match-p "v=module123"
                              (plist-get caption-info :caption_url))))))

(provide 'org-defuddle-test)

;;; org-defuddle-test.el ends here
