;;; org-defuddle.el --- Extract readable web pages into Org -*- lexical-binding: t; -*-

;; Copyright (C) 2026
;; SPDX-License-Identifier: MIT

;; Author: Lucius Chen
;; Version: 0.1.3
;; Package-Requires: ((emacs "27.1"))
;; Keywords: hypermedia, outlines, tools

;;; Commentary:

;; Emacs frontend for org-defuddle's Rust dynamic module.

;;; Code:

(require 'json)
(require 'subr-x)
(require 'url)
(require 'url-parse)
(require 'url-util)

(declare-function denote "denote"
                  (&optional title keywords file-type directory date template
                             signature identifier))
(declare-function org-roam-capture- "org-roam-capture" (&rest args))
(declare-function org-roam-node-create "org-roam-node" (&rest args))

(declare-function org-defuddle-module-parse-json "org-defuddle-module" (html url))
(declare-function org-defuddle-module-parse-json-with-options
                  "org-defuddle-module"
                  (html url include-images remove-small-images
                        content-selector include-replies remove-hidden-elements
                        remove-exact-selectors remove-partial-selectors
                        remove-content-patterns remove-low-scoring
                        standardize debug profile frontmatter markdown
                        separate-markdown))
(declare-function org-defuddle-module-parse-org "org-defuddle-module" (html url))
(declare-function org-defuddle-module-parse-property
                  "org-defuddle-module"
                  (html url property))
(declare-function org-defuddle-module-parse-org-with-options
                  "org-defuddle-module"
                  (html url include-images remove-small-images
                        content-selector include-replies remove-hidden-elements
                        remove-exact-selectors remove-partial-selectors
                        remove-content-patterns remove-low-scoring
                        standardize debug profile frontmatter markdown
                        separate-markdown))
(declare-function org-defuddle-module-parse-property-with-options
                  "org-defuddle-module"
                  (html url property include-images remove-small-images
                        content-selector include-replies remove-hidden-elements
                        remove-exact-selectors remove-partial-selectors
                        remove-content-patterns remove-low-scoring
                        standardize debug profile frontmatter markdown
                        separate-markdown))
(declare-function org-defuddle-module-parse-c2-json "org-defuddle-module" (json url))
(declare-function org-defuddle-module-parse-c2-org "org-defuddle-module" (json url))
(declare-function org-defuddle-module-parse-x-oembed-json
                  "org-defuddle-module"
                  (json url))
(declare-function org-defuddle-module-parse-x-oembed-org
                  "org-defuddle-module"
                  (json url))
(declare-function org-defuddle-module-parse-fxtwitter-json
                  "org-defuddle-module"
                  (json url))
(declare-function org-defuddle-module-parse-fxtwitter-org
                  "org-defuddle-module"
                  (json url))
(declare-function org-defuddle-module-bilibili-video-info
                  "org-defuddle-module"
                  (view-json url))
(declare-function org-defuddle-module-bilibili-subtitle-info
                  "org-defuddle-module"
                  (player-json preferred-language))
(declare-function org-defuddle-module-parse-bilibili-json
                  "org-defuddle-module"
                  (view-json subtitle-json url language-code))
(declare-function org-defuddle-module-parse-bilibili-org
                  "org-defuddle-module"
                  (view-json subtitle-json url language-code))
(declare-function org-defuddle-module-youtube-caption-info
                  "org-defuddle-module"
                  (player-json preferred-language))
(declare-function org-defuddle-module-youtube-inline-caption-info
                  "org-defuddle-module"
                  (html url preferred-language))
(declare-function org-defuddle-module-parse-youtube-json
                  "org-defuddle-module"
                  (player-json caption-xml chapters-json url language-code))
(declare-function org-defuddle-module-parse-youtube-org
                  "org-defuddle-module"
                  (player-json caption-xml chapters-json url language-code))

(defvar url-request-extra-headers)
(defvar url-request-method)
(defvar url-request-data)

(defconst org-defuddle--source-directory
  (file-name-directory (or load-file-name buffer-file-name default-directory))
  "Directory containing org-defuddle.el.")

(defconst org-defuddle--module-version "v0.1.3"
  "GitHub release containing the native module required by this package.")

(defconst org-defuddle--github-release-url
  "https://github.com/LuciusChen/org-defuddle/releases"
  "Base URL for org-defuddle GitHub releases.")

(defun org-defuddle--installed-module-file ()
  "Return the default per-user native module path."
  (expand-file-name
   (concat "liborg_defuddle_module-" org-defuddle--module-version
           module-file-suffix)
   (expand-file-name "modules" user-emacs-directory)))

(defgroup org-defuddle nil
  "Extract readable web pages into Org using a Rust dynamic module."
  :group 'org
  :prefix "org-defuddle-")

(defcustom org-defuddle-module-file (org-defuddle--installed-module-file)
  "Path to the compiled org-defuddle dynamic module.

The default is a persistent directory under `user-emacs-directory'.
`org-defuddle-download-module' installs the pre-built GitHub release
asset here.  A repository-local Cargo release build remains available
as a development fallback when this file does not exist."
  :type 'file
  :group 'org-defuddle)

(defcustom org-defuddle-cli-file nil
  "Path to the compiled org-defuddle CLI.

When nil, URL extraction tries the repository-local Cargo release
output when `org-defuddle-use-cli-url-fetch' is non-nil."
  :type '(choice (const :tag "Auto-detect" nil) file)
  :group 'org-defuddle)

(defcustom org-defuddle-use-cli-url-fetch t
  "Whether URL extraction may use the Rust CLI fetch stack.

Generic HTML URL extraction uses the CLI path only when requested
options can be represented without changing output semantics.  Site
specific API helper requests use the CLI fetch subcommand when it is
available, and fall back to Emacs `url-retrieve' if the fetch process
fails."
  :type 'boolean
  :group 'org-defuddle)

(defcustom org-defuddle-output-backend 'buffer
  "Where completed Org extraction results are delivered.

The default `buffer' preserves the existing behavior.  `denote' creates an
Org note through Denote, while `org-roam' creates one through Org-roam.  The
selected package is loaded only when its backend is used."
  :type '(choice (const :tag "Temporary Org buffer" buffer)
                 (const :tag "Denote note" denote)
                 (const :tag "Org-roam note" org-roam))
  :group 'org-defuddle)

(defcustom org-defuddle-note-keywords nil
  "Keywords assigned to notes created by a note output backend."
  :type '(repeat string)
  :group 'org-defuddle)

(defcustom org-defuddle-denote-directory nil
  "Denote directory used for extracted notes.

When nil, use Denote's configured default directory."
  :type '(choice (const :tag "Denote default" nil) directory)
  :group 'org-defuddle)

(defcustom org-defuddle-org-roam-file-name
  "%<%Y%m%d%H%M%S>-${slug}.org"
  "Org-roam capture target used for extracted notes."
  :type 'string
  :group 'org-defuddle)

(defcustom org-defuddle-include-images t
  "Whether org-defuddle keeps images in extracted Org output by default."
  :type 'boolean
  :group 'org-defuddle)

(defcustom org-defuddle-remove-small-images t
  "Whether org-defuddle removes small images and unresolved placeholders."
  :type 'boolean
  :group 'org-defuddle)

(defcustom org-defuddle-content-selector nil
  "CSS selector used as the main content element by default.

When nil or when the selector does not match, org-defuddle falls back to
automatic main-content detection."
  :type '(choice (const :tag "Auto-detect" nil) string)
  :group 'org-defuddle)

(defcustom org-defuddle-include-replies 'extractors
  "Whether org-defuddle includes replies and comments by default.

The value `extractors' includes replies from site-specific extractors.
The value t requests all supported replies.  The value nil omits
replies."
  :type '(choice (const :tag "Site-specific extractor replies" extractors)
                 (const :tag "All supported replies" t)
                 (const :tag "No replies" nil))
  :group 'org-defuddle)

(defcustom org-defuddle-language nil
  "Preferred language code for API transcript extractors.

When nil, org-defuddle uses the upstream extractor's stable default
language order."
  :type '(choice (const :tag "Extractor default" nil) string)
  :group 'org-defuddle)

(defcustom org-defuddle-use-async t
  "Whether URL extraction may use site-specific API and alternate-host fetches."
  :type 'boolean
  :group 'org-defuddle)

(defcustom org-defuddle-standardize t
  "Whether org-defuddle standardizes HTML before rendering Org."
  :type 'boolean
  :group 'org-defuddle)

(defcustom org-defuddle-debug nil
  "Whether org-defuddle includes debug extraction information by default."
  :type 'boolean
  :group 'org-defuddle)

(defcustom org-defuddle-profile nil
  "Whether org-defuddle includes per-step profiling timings by default."
  :type 'boolean
  :group 'org-defuddle)

(defcustom org-defuddle-frontmatter nil
  "Whether org-defuddle prepends YAML frontmatter to Org output by default."
  :type 'boolean
  :group 'org-defuddle)

(defcustom org-defuddle-markdown nil
  "Whether org-defuddle includes Markdown content by default."
  :type 'boolean
  :group 'org-defuddle)

(defcustom org-defuddle-separate-markdown nil
  "Whether org-defuddle populates separate Markdown content by default."
  :type 'boolean
  :group 'org-defuddle)

(defcustom org-defuddle-remove-hidden-elements t
  "Whether org-defuddle removes hidden elements by default."
  :type 'boolean
  :group 'org-defuddle)

(defcustom org-defuddle-remove-exact-selectors t
  "Whether org-defuddle removes exact selector matches by default."
  :type 'boolean
  :group 'org-defuddle)

(defcustom org-defuddle-remove-partial-selectors t
  "Whether org-defuddle removes partial selector matches by default."
  :type 'boolean
  :group 'org-defuddle)

(defcustom org-defuddle-remove-content-patterns t
  "Whether org-defuddle removes known content-pattern noise by default."
  :type 'boolean
  :group 'org-defuddle)

(defcustom org-defuddle-remove-low-scoring t
  "Whether org-defuddle removes low-scoring content blocks by default."
  :type 'boolean
  :group 'org-defuddle)

(defvar org-defuddle--module-loaded nil)

(defconst org-defuddle--module-functions
  '(org-defuddle-module-parse-json
    org-defuddle-module-parse-json-with-options
    org-defuddle-module-parse-org
    org-defuddle-module-parse-property
    org-defuddle-module-parse-org-with-options
    org-defuddle-module-parse-property-with-options
    org-defuddle-module-parse-c2-json
    org-defuddle-module-parse-c2-org
    org-defuddle-module-parse-x-oembed-json
    org-defuddle-module-parse-x-oembed-org
    org-defuddle-module-parse-fxtwitter-json
    org-defuddle-module-parse-fxtwitter-org
    org-defuddle-module-bilibili-video-info
    org-defuddle-module-bilibili-subtitle-info
    org-defuddle-module-parse-bilibili-json
    org-defuddle-module-parse-bilibili-org
    org-defuddle-module-youtube-caption-info
    org-defuddle-module-parse-youtube-json
    org-defuddle-module-parse-youtube-org)
  "Functions that must be provided by the Rust dynamic module.")

(defconst org-defuddle--optional-module-functions
  '(org-defuddle-module-youtube-inline-caption-info)
  "Functions used when provided by a newer Rust dynamic module.")

(defun org-defuddle--repo-root ()
  "Return the repository root when this file is loaded from the source tree."
  org-defuddle--source-directory)

(defun org-defuddle--default-module-file ()
  "Return the repository-local dynamic module path for development."
  (let* ((root (org-defuddle--repo-root))
         (lib (cond
               ((eq system-type 'darwin) "liborg_defuddle_module.dylib")
               ((eq system-type 'windows-nt) "org_defuddle_module.dll")
               (t "liborg_defuddle_module.so"))))
    (expand-file-name (concat "target/release/" lib) root)))

(defun org-defuddle--existing-module-file ()
  "Return an installed or repository-local dynamic module path."
  (cond
   ((and org-defuddle-module-file
         (file-exists-p org-defuddle-module-file))
    org-defuddle-module-file)
   ((file-exists-p (org-defuddle--default-module-file))
    (org-defuddle--default-module-file))))

(defun org-defuddle--module-architecture ()
  "Return the Rust target architecture for this Emacs process."
  (cond
   ((string-match-p "aarch64\\|arm64" system-configuration) "aarch64")
   ((string-match-p "x86_64" system-configuration) "x86_64")
   (t (user-error "Unsupported architecture: %s" system-configuration))))

(defun org-defuddle--module-target-triple ()
  "Return the release target triple for this Emacs process."
  (let ((architecture (org-defuddle--module-architecture)))
    (cond
     ((eq system-type 'darwin)
      (concat architecture "-apple-darwin"))
     ((eq system-type 'gnu/linux)
      (concat architecture "-unknown-linux-gnu"))
     ((eq system-type 'windows-nt)
      (unless (string= architecture "x86_64")
        (user-error "Unsupported Windows architecture: %s" architecture))
      "x86_64-pc-windows-msvc")
     (t (user-error "Unsupported platform: %s" system-type)))))

(defun org-defuddle--module-release-asset ()
  "Return the pre-built release asset name for this platform."
  (format "liborg-defuddle-%s%s"
          (org-defuddle--module-target-triple)
          module-file-suffix))

(defun org-defuddle--module-download-url ()
  "Return the pinned pre-built module download URL for this package."
  (format "%s/download/%s/%s"
          org-defuddle--github-release-url
          org-defuddle--module-version
          (org-defuddle--module-release-asset)))

(defun org-defuddle--module-checksums-url ()
  "Return the checksum manifest URL for the pinned module release."
  (format "%s/download/%s/SHA256SUMS"
          org-defuddle--github-release-url
          org-defuddle--module-version))

(defun org-defuddle--file-sha256 (file)
  "Return the hexadecimal SHA-256 digest of FILE."
  (with-temp-buffer
    (set-buffer-multibyte nil)
    (insert-file-contents-literally file)
    (secure-hash 'sha256 (current-buffer))))

(defun org-defuddle--expected-module-sha256 (manifest)
  "Return this platform module's SHA-256 digest from MANIFEST."
  (let ((case-fold-search t)
        (asset (org-defuddle--module-release-asset)))
    (with-temp-buffer
      (insert manifest)
      (goto-char (point-min))
      (if (re-search-forward
           (format "^\\([[:xdigit:]]\\{64\\}\\)[ \t]+\\*?%s\r?$"
                   (regexp-quote asset))
           nil t)
          (downcase (match-string 1))
        (error "Checksum manifest has no entry for %s" asset)))))

(defun org-defuddle--verify-module-checksum (module-file manifest-file)
  "Verify MODULE-FILE against its entry in MANIFEST-FILE."
  (let* ((manifest (with-temp-buffer
                     (insert-file-contents manifest-file)
                     (buffer-string)))
         (expected (org-defuddle--expected-module-sha256 manifest))
         (actual (org-defuddle--file-sha256 module-file)))
    (unless (string-equal expected actual)
      (error "Org-defuddle module checksum mismatch: expected %s, got %s"
             expected actual))))

(defun org-defuddle--verify-module-functions (module-file)
  "Verify that MODULE-FILE provided every required module function."
  (dolist (function org-defuddle--module-functions)
    (unless (fboundp function)
      (error "Org-defuddle module at %s is missing entry point %s"
             module-file function))))

(defun org-defuddle--load-module-file (module-file)
  "Load and verify the dynamic module at MODULE-FILE."
  (module-load module-file)
  (org-defuddle--verify-module-functions module-file)
  (setq org-defuddle--module-loaded t))

(defun org-defuddle--default-cli-file ()
  "Return the default org-defuddle CLI path for the current platform."
  (expand-file-name
   (concat "target/release/org-defuddle"
           (if (eq system-type 'windows-nt) ".exe" ""))
   (org-defuddle--repo-root)))

(defun org-defuddle--cli-file ()
  "Return an executable org-defuddle CLI path, or nil."
  (let ((cli-file (or org-defuddle-cli-file
                      (org-defuddle--default-cli-file))))
    (and (file-executable-p cli-file) cli-file)))

;;;###autoload
(defun org-defuddle-download-module (&optional path)
  "Download and load the pinned pre-built Rust module.

Install the module at PATH, defaulting to `org-defuddle-module-file'."
  (interactive)
  (setq path (expand-file-name
              (or path org-defuddle-module-file
                  (org-defuddle--installed-module-file))))
  (let* ((directory (file-name-directory path))
         (url (org-defuddle--module-download-url))
         (checksums-url (org-defuddle--module-checksums-url))
         (temporary-file nil)
         (checksums-file nil))
    (unless (string-prefix-p "https://" url)
      (error "Refusing non-HTTPS module download URL: %s" url))
    (unless (string-prefix-p "https://" checksums-url)
      (error "Refusing non-HTTPS checksum download URL: %s" checksums-url))
    (make-directory directory t)
    (setq temporary-file
          (make-temp-file (expand-file-name ".org-defuddle-module-" directory))
          checksums-file
          (make-temp-file (expand-file-name ".org-defuddle-checksums-" directory)))
    (unwind-protect
        (progn
          (message "Downloading org-defuddle module from %s..." url)
          (url-copy-file url temporary-file t)
          (url-copy-file checksums-url checksums-file t)
          (org-defuddle--verify-module-checksum temporary-file checksums-file)
          (rename-file temporary-file path t)
          (setq temporary-file nil)
          (if org-defuddle--module-loaded
              (message "Org-defuddle module installed at %s; restart Emacs to load it"
                       path)
            (org-defuddle--load-module-file path)
            (message "Org-defuddle module installed and loaded from %s" path)))
      (when (and temporary-file (file-exists-p temporary-file))
        (delete-file temporary-file))
      (when (and checksums-file (file-exists-p checksums-file))
        (delete-file checksums-file)))))

;;;###autoload
(defun org-defuddle-load-module (&optional offer-download)
  "Load the Rust dynamic module.

When OFFER-DOWNLOAD is non-nil and no module is installed, ask whether
to download the pinned pre-built GitHub release.  Interactive calls set
OFFER-DOWNLOAD automatically."
  (interactive (list t))
  (unless org-defuddle--module-loaded
    (if-let* ((module-file (org-defuddle--existing-module-file)))
        (org-defuddle--load-module-file module-file)
      (if (and offer-download
               (yes-or-no-p
                "Org-defuddle module not found; download pre-built release? "))
          (org-defuddle-download-module)
        (user-error
         "Org-defuddle module not found; run M-x org-defuddle-download-module")))))

(defun org-defuddle--option (options key default)
  "Return OPTIONS value for KEY, or DEFAULT when KEY is absent."
  (if (and options (plist-member options key))
      (plist-get options key)
    default))

(defun org-defuddle--content-selector-option (options)
  "Return the content selector string requested by OPTIONS."
  (let ((selector (org-defuddle--option options
                                        :content-selector
                                        org-defuddle-content-selector)))
    (cond
     ((null selector) "")
     ((stringp selector) selector)
     (t (user-error ":content-selector must be a string or nil")))))

(defun org-defuddle--include-replies-option (options)
  "Return the include-replies mode string requested by OPTIONS."
  (let ((include-replies (org-defuddle--option options
                                               :include-replies
                                               org-defuddle-include-replies)))
    (cond
     ((eq include-replies 'extractors) "extractors")
     ((eq include-replies t) "all")
     ((null include-replies) "none")
     (t (user-error ":include-replies must be t, nil, or `extractors'")))))

(defun org-defuddle--include-images-option (options)
  "Return whether OPTIONS request image inclusion.

`:remove-images' is the upstream-compatible inverse of
`:include-images' and takes precedence when both are present."
  (let ((remove-images (and options
                            (plist-member options :remove-images)
                            (plist-get options :remove-images))))
    (cond
     ((and options
           (plist-member options :remove-images)
           (not (memq remove-images '(nil t))))
      (user-error ":remove-images must be t or nil"))
     ((and options (plist-member options :remove-images))
      (not remove-images))
     (t (not (null (org-defuddle--option options
                                         :include-images
                                         org-defuddle-include-images)))))))

(defun org-defuddle--language-option (options)
  "Return the preferred language string requested by OPTIONS."
  (let ((language (org-defuddle--option options
                                        :language
                                        org-defuddle-language)))
    (cond
     ((null language) "")
     ((stringp language) language)
     (t (user-error ":language must be a string or nil")))))

(defun org-defuddle--use-async-option (options)
  "Return whether OPTIONS permit async URL extraction."
  (let ((use-async (org-defuddle--option options
                                         :use-async
                                         org-defuddle-use-async)))
    (cond
     ((eq use-async t) t)
     ((null use-async) nil)
     (t (user-error ":use-async must be t or nil")))))

(defun org-defuddle--standardize-option (options)
  "Return whether OPTIONS request HTML standardization."
  (let ((standardize (org-defuddle--option options
                                           :standardize
                                           org-defuddle-standardize)))
    (cond
     ((eq standardize t) t)
     ((null standardize) nil)
     (t (user-error ":standardize must be t or nil")))))

(defun org-defuddle--debug-option (options)
  "Return whether OPTIONS request debug extraction information."
  (let ((debug (org-defuddle--option options :debug org-defuddle-debug)))
    (cond
     ((eq debug t) t)
     ((null debug) nil)
     (t (user-error ":debug must be t or nil")))))

(defun org-defuddle--profile-option (options)
  "Return whether OPTIONS request per-step profiling timings."
  (let ((profile (org-defuddle--option options :profile org-defuddle-profile)))
    (cond
     ((eq profile t) t)
     ((null profile) nil)
     (t (user-error ":profile must be t or nil")))))

(defun org-defuddle--frontmatter-option (options)
  "Return whether OPTIONS request YAML frontmatter output."
  (let ((frontmatter (org-defuddle--option options
                                           :frontmatter
                                           org-defuddle-frontmatter)))
    (cond
     ((eq frontmatter t) t)
     ((null frontmatter) nil)
     (t (user-error ":frontmatter must be t or nil")))))

(defun org-defuddle--markdown-property-p (property)
  "Return non-nil when PROPERTY requires Markdown rendering."
  (member property '("contentMarkdown" "content_markdown" "markdown")))

(defun org-defuddle--markdown-option (options &optional property)
  "Return whether OPTIONS or PROPERTY request Markdown output."
  (let ((markdown (org-defuddle--option options
                                        :markdown
                                        org-defuddle-markdown)))
    (cond
     ((not (memq markdown '(nil t)))
      (user-error ":markdown must be t or nil"))
     ((and property (org-defuddle--markdown-property-p property)) t)
     (t markdown))))

(defun org-defuddle--separate-markdown-option (options &optional property)
  "Return whether OPTIONS or PROPERTY request separate Markdown content."
  (let ((separate-markdown
         (if (and options (plist-member options :separateMarkdown))
             (plist-get options :separateMarkdown)
           (org-defuddle--option options
                                 :separate-markdown
                                 org-defuddle-separate-markdown))))
    (cond
     ((not (memq separate-markdown '(nil t)))
      (user-error ":separate-markdown must be t or nil"))
     ((and property (org-defuddle--markdown-property-p property)) t)
     (t separate-markdown))))

(defun org-defuddle--c2-page-title (url)
  "Return the C2 Wiki page title from URL, or nil for non-C2 URLs."
  (let* ((parsed (ignore-errors (url-generic-parse-url url)))
         (host (and parsed (url-host parsed))))
    (when (and host
               (member (downcase host) '("wiki.c2.com" "c2.com")))
      (let ((filename (or (url-filename parsed) "")))
        (if (string-match "[?&]\\([A-Za-z][[:alnum:]_]*\\)" filename)
            (match-string 1 filename)
          "WelcomeVisitors")))))

(defun org-defuddle--c2-api-url (title)
  "Return the C2 Wiki API URL for TITLE."
  (concat "https://c2.com/wiki/remodel/pages/"
          (url-hexify-string title)))

(defun org-defuddle--x-status-match (url)
  "Return (USER . ID) when URL is an X/Twitter status or article URL."
  (let* ((parsed (ignore-errors (url-generic-parse-url url)))
         (host (and parsed (url-host parsed)))
         (path (and parsed (url-filename parsed))))
    (when (and host
               (member (downcase host)
                       '("x.com" "twitter.com" "mobile.twitter.com"))
               path
               (string-match
                "\\`/\\([A-Za-z0-9_][A-Za-z0-9_]*\\)/\\(?:status\\|article\\)/\\([0-9]+\\)"
                path))
      (cons (match-string 1 path) (match-string 2 path)))))

(defun org-defuddle--fxtwitter-api-url (match)
  "Return the FxTwitter API URL for MATCH from `org-defuddle--x-status-match'."
  (format "https://api.fxtwitter.com/%s/status/%s"
          (url-hexify-string (car match))
          (url-hexify-string (cdr match))))

(defun org-defuddle--x-oembed-api-url (url)
  "Return the X/Twitter oEmbed API URL for URL."
  (concat "https://publish.twitter.com/oembed?url="
          (url-hexify-string url)
          "&omit_script=true"))

(defun org-defuddle--bilibili-bvid (url)
  "Return the Bilibili BV id from URL, or nil for non-Bilibili URLs."
  (let* ((parsed (ignore-errors (url-generic-parse-url url)))
         (host (and parsed (url-host parsed)))
         (path (and parsed (url-filename parsed))))
    (when (and host
               (or (string= (downcase host) "bilibili.com")
                   (string-suffix-p ".bilibili.com" (downcase host)))
               path
               (string-match "/video/\\(BV[0-9A-Za-z]+\\)/?" path))
      (match-string 1 path))))

(defun org-defuddle--bilibili-view-api-url (bvid)
  "Return the Bilibili view API URL for BVID."
  (concat "https://api.bilibili.com/x/web-interface/view?bvid="
          (url-hexify-string bvid)))

(defun org-defuddle--bilibili-player-api-urls (info)
  "Return Bilibili player API URLs for parsed video INFO."
  (let ((bvid (plist-get info :bvid))
        (aid (plist-get info :aid))
        (cid (plist-get info :cid)))
    (when (and bvid aid cid
               (not (string= bvid ""))
               (> aid 0)
               (> cid 0))
      (list
       (format "https://api.bilibili.com/x/player/wbi/v2?bvid=%s&aid=%s&cid=%s"
               (url-hexify-string bvid) aid cid)
       (format "https://api.bilibili.com/x/player/v2?bvid=%s&cid=%s"
               (url-hexify-string bvid) cid)
       (format "https://api.bilibili.com/x/player/v2?aid=%s&cid=%s" aid cid)))))

(defun org-defuddle--youtube-video-id (url)
  "Return the YouTube video id from URL, or nil for non-YouTube URLs."
  (let* ((parsed (ignore-errors (url-generic-parse-url url)))
         (host (and parsed (url-host parsed)))
         (filename (and parsed (url-filename parsed)))
         (host (and host (downcase host))))
    (cond
     ((and (string= host "youtu.be")
           filename
           (string-match "\\`/\\([^/?#]+\\)" filename))
      (match-string 1 filename))
     ((and host
           (or (string= host "youtube.com")
               (string-suffix-p ".youtube.com" host)
               (string= host "youtube-nocookie.com")
               (string-suffix-p ".youtube-nocookie.com" host))
           filename
           (or (string-match
                "/\\(?:shorts\\|embed\\|live\\)/\\([^/?#]+\\)"
                filename)
               (string-match "[?&]v=\\([^&#]+\\)" filename)))
      (match-string 1 filename)))))

(defun org-defuddle--youtube-player-api-url ()
  "Return the YouTube Innertube player API URL."
  "https://www.youtube.com/youtubei/v1/player?prettyPrint=false")

(defun org-defuddle--youtube-next-api-url ()
  "Return the YouTube Innertube next API URL."
  "https://www.youtube.com/youtubei/v1/next?prettyPrint=false")

(defun org-defuddle--youtube-player-configs ()
  "Return YouTube Innertube client configurations in fallback order."
  '((:name "IOS" :version "20.10.3")
    (:name "ANDROID"
     :version "20.10.38"
     :user-agent "com.google.android.youtube/20.10.38 (Linux; U; Android 14)")
    (:name "WEB" :version "2.20240101.00.00")))

(defun org-defuddle--youtube-request-body (video-id client-name client-version)
  "Return an Innertube request body for VIDEO-ID.

CLIENT-NAME and CLIENT-VERSION identify the Innertube client."
  (json-encode
   `(("context" .
      (("client" .
        (("clientName" . ,client-name)
         ("clientVersion" . ,client-version)))))
     ("videoId" . ,video-id))))

(defun org-defuddle--youtube-api-headers (language &optional user-agent)
  "Return YouTube API headers for LANGUAGE and optional USER-AGENT."
  (append
   '(("Content-Type" . "application/json"))
   (when (and language (not (string= language "")))
     `(("Accept-Language" . ,language)))
   (when user-agent
     `(("User-Agent" . ,user-agent)))))

(defun org-defuddle--reddit-old-comments-url (url)
  "Return an old.reddit.com URL for Reddit comments URL, or nil."
  (let* ((parsed (ignore-errors (url-generic-parse-url url)))
         (host (and parsed (url-host parsed)))
         (filename (and parsed (url-filename parsed)))
         (scheme (and parsed (url-type parsed)))
         (host (and host (downcase host))))
    (when (and host
               (not (string= host "old.reddit.com"))
               (or (string= host "reddit.com")
                   (string-suffix-p ".reddit.com" host))
               filename
               (string-match-p "/r/[^/]+/comments/[^/]+" filename))
      (format "%s://old.reddit.com%s" (or scheme "https") filename))))

(defun org-defuddle--response-body ()
  "Return the current `url-retrieve' buffer response body."
  (goto-char (point-min))
  (re-search-forward "\r?\n\r?\n" nil 'move)
  (buffer-substring-no-properties (point) (point-max)))

(defun org-defuddle--retrieve-body-with-url (url callback headers method data)
  "Fetch URL with `url-retrieve' and call CALLBACK with the response body.

HEADERS, METHOD, and DATA configure the request."
  (let ((url-request-extra-headers headers)
        (url-request-method (or method "GET"))
        (url-request-data data))
    (url-retrieve
     url
     (lambda (_status)
       (unwind-protect
           (funcall callback (org-defuddle--response-body))
         (kill-buffer (current-buffer)))))))

(defun org-defuddle--cli-fetch-command (url headers method data)
  "Return a CLI fetch command for URL using HEADERS, METHOD, and DATA."
  (let ((command (list (org-defuddle--cli-file) "fetch" url)))
    (when (and method (not (string= (upcase method) "GET")))
      (setq command (append command (list "--method" method))))
    (dolist (header headers)
      (setq command
            (append command
                    (list "--header"
                          (format "%s: %s" (car header) (cdr header))))))
    (when data
      (setq command (append command (list "--data" data))))
    command))

(defun org-defuddle--retrieve-body-with-cli (url callback headers method data)
  "Fetch URL with the Rust CLI and call CALLBACK with the response body.

HEADERS, METHOD, and DATA configure the request.  On CLI failure, fall
back to `org-defuddle--retrieve-body-with-url'."
  (let* ((buffer (generate-new-buffer " *org-defuddle-fetch*"))
         (command (org-defuddle--cli-fetch-command url headers method data)))
    (make-process
     :name "org-defuddle-fetch"
     :buffer buffer
     :command command
     :noquery t
     :sentinel
     (lambda (process _event)
       (when (memq (process-status process) '(exit signal))
         (let* ((exit-code (process-exit-status process))
                (output-buffer (process-buffer process))
                (body (when (buffer-live-p output-buffer)
                        (with-current-buffer output-buffer
                          (buffer-substring-no-properties
                           (point-min)
                           (point-max))))))
           (when (buffer-live-p output-buffer)
             (kill-buffer output-buffer))
           (if (zerop exit-code)
               (funcall callback (or body ""))
             (org-defuddle--retrieve-body-with-url
              url callback headers method data))))))))

(defun org-defuddle--retrieve-body (url callback &optional headers method data)
  "Fetch URL and call CALLBACK with the response body.

HEADERS, METHOD, and DATA configure the request."
  (if (and org-defuddle-use-cli-url-fetch
           (org-defuddle--cli-file))
      (org-defuddle--retrieve-body-with-cli
       url callback headers method data)
    (org-defuddle--retrieve-body-with-url
     url callback headers method data)))

(defun org-defuddle--cli-compatible-html-url-options-p (options)
  "Return non-nil when OPTIONS can use the CLI URL backend."
  (and org-defuddle-use-cli-url-fetch
       (org-defuddle--cli-file)
       (org-defuddle--include-images-option options)
       (org-defuddle--option options
                             :remove-small-images
                             org-defuddle-remove-small-images)
       (null (org-defuddle--option options
                                   :content-selector
                                   org-defuddle-content-selector))
       (eq (org-defuddle--option options
                                 :include-replies
                                 org-defuddle-include-replies)
           'extractors)
       (org-defuddle--option options
                             :remove-hidden-elements
                             org-defuddle-remove-hidden-elements)
       (org-defuddle--option options
                             :remove-exact-selectors
                             org-defuddle-remove-exact-selectors)
       (org-defuddle--option options
                             :remove-partial-selectors
                             org-defuddle-remove-partial-selectors)
       (org-defuddle--option options
                             :remove-content-patterns
                             org-defuddle-remove-content-patterns)
       (org-defuddle--option options
                             :remove-low-scoring
                             org-defuddle-remove-low-scoring)
       (org-defuddle--standardize-option options)
       (not (org-defuddle--debug-option options))
       (not (org-defuddle--profile-option options))
       (not (org-defuddle--markdown-option options))
       (not (org-defuddle--separate-markdown-option options))))

(defun org-defuddle--cli-url-command (url options)
  "Return a CLI command list for parsing URL using OPTIONS."
  (let ((command (list (org-defuddle--cli-file) "parse" url))
        (language (org-defuddle--language-option options)))
    (when (org-defuddle--frontmatter-option options)
      (setq command (append command (list "--frontmatter"))))
    (unless (string= language "")
      (setq command (append command (list "--lang" language))))
    command))

(defun org-defuddle--cli-url-to-org (url options)
  "Fetch and parse URL through the Rust CLI using OPTIONS."
  (let* ((buffer (generate-new-buffer " *org-defuddle-cli*"))
         (command (org-defuddle--cli-url-command url options)))
    (make-process
     :name "org-defuddle-cli"
     :buffer buffer
     :command command
     :noquery t
     :sentinel
     (lambda (process _event)
       (when (memq (process-status process) '(exit signal))
         (let* ((exit-code (process-exit-status process))
                (output-buffer (process-buffer process))
                (output (when (buffer-live-p output-buffer)
                          (with-current-buffer output-buffer
                            (buffer-substring-no-properties
                             (point-min)
                             (point-max))))))
           (when (buffer-live-p output-buffer)
             (kill-buffer output-buffer))
           (if (zerop exit-code)
               (org-defuddle--insert-org-buffer
                (string-trim-right (or output "")))
             (message "org-defuddle CLI failed: %s"
                      (string-trim (or output ""))))))))))

(defun org-defuddle--org-title (org)
  "Return the first headline title from ORG."
  (if (string-match "\\`[[:space:]]*\\*+[ \t]+\\([^\n]+\\)" org)
      (string-trim (match-string 1 org))
    "Web capture"))

(defun org-defuddle--insert-temporary-org-buffer (org)
  "Insert ORG into a new temporary Org buffer and display it."
  (with-current-buffer (generate-new-buffer "*org-defuddle*")
    (insert org)
    (org-mode)
    (pop-to-buffer (current-buffer))))

(defun org-defuddle--insert-denote-note (org)
  "Create and display a Denote note containing ORG."
  (unless (require 'denote nil t)
    (user-error "Denote is not installed"))
  (let* ((title (org-defuddle--org-title org))
         (path (denote title
                       org-defuddle-note-keywords
                       'org
                       org-defuddle-denote-directory))
         (buffer (find-file-noselect path)))
    (with-current-buffer buffer
      (goto-char (point-max))
      (unless (bolp)
        (insert "\n"))
      (insert "\n" org)
      (save-buffer))
    (pop-to-buffer buffer)))

(defun org-defuddle--insert-org-roam-note (org)
  "Create and display an Org-roam note containing ORG."
  (unless (require 'org-roam-capture nil t)
    (user-error "Org-roam is not installed"))
  (let* ((title (org-defuddle--org-title org))
         (content (replace-regexp-in-string "%" "%%" org t t))
         (template
          (list
           (list "d" "org-defuddle" 'plain
                 (list 'function (lambda () content))
                 :target
                 (list 'file+head
                       org-defuddle-org-roam-file-name
                       "#+title: ${title}\n#+filetags: ${org-defuddle-tags}\n\n")
                 :immediate-finish t
                 :jump-to-captured t
                 :unnarrowed t))))
    (org-roam-capture-
     :node (org-roam-node-create :title title)
     :info (list :org-defuddle-tags
                 (mapconcat (lambda (keyword) (format ":%s:" keyword))
                            org-defuddle-note-keywords
                            ""))
     :templates template)))

(defun org-defuddle--insert-org-buffer (org)
  "Deliver ORG according to `org-defuddle-output-backend'."
  (pcase org-defuddle-output-backend
    ('buffer (org-defuddle--insert-temporary-org-buffer org))
    ('denote (org-defuddle--insert-denote-note org))
    ('org-roam (org-defuddle--insert-org-roam-note org))
    (_ (user-error "Unsupported org-defuddle output backend: %S"
                   org-defuddle-output-backend))))

(defun org-defuddle-parse-html (html &optional url options)
  "Parse HTML and return a plist containing extracted metadata and Org.

The returned plist includes upstream-compatible keys such as
`:content', `:title', `:wordCount', `:parseTime', and
`:contentMarkdown', plus upstream response extras `:schemaOrgData'
`:metaTags', `:extractorType', and `:variables'.  It also includes
org-defuddle extensions such as `:html', `:org', `:frontmatter',
`:word_count', and `:parse_time'.

URL is used for metadata and relative URL resolution.  OPTIONS is a
plist.  `:include-images' overrides `org-defuddle-include-images',
`:remove-images' is accepted as its upstream-compatible inverse,
`:remove-small-images' overrides `org-defuddle-remove-small-images',
`:content-selector' overrides `org-defuddle-content-selector', and
`:include-replies' overrides `org-defuddle-include-replies'.
`:remove-hidden-elements' overrides
`org-defuddle-remove-hidden-elements'.  `:remove-exact-selectors'
and `:remove-partial-selectors' override their matching defcustoms.
`:remove-content-patterns' overrides
`org-defuddle-remove-content-patterns'.  `:remove-low-scoring'
overrides `org-defuddle-remove-low-scoring'.  `:standardize'
overrides `org-defuddle-standardize'.  `:debug' overrides
`org-defuddle-debug'.  `:profile' overrides `org-defuddle-profile'.
`:frontmatter' overrides `org-defuddle-frontmatter'.  `:markdown'
overrides `org-defuddle-markdown'.  `:separate-markdown' and
`:separateMarkdown' override `org-defuddle-separate-markdown'."
  (org-defuddle-load-module)
  (let ((include-images (org-defuddle--include-images-option options))
        (remove-small-images (org-defuddle--option
                              options
                              :remove-small-images
                              org-defuddle-remove-small-images))
        (content-selector (org-defuddle--content-selector-option options))
        (include-replies (org-defuddle--include-replies-option options))
        (remove-hidden-elements (org-defuddle--option
                                 options
                                 :remove-hidden-elements
                                 org-defuddle-remove-hidden-elements))
        (remove-exact-selectors (org-defuddle--option
                                 options
                                 :remove-exact-selectors
                                 org-defuddle-remove-exact-selectors))
        (remove-partial-selectors (org-defuddle--option
                                   options
                                   :remove-partial-selectors
                                   org-defuddle-remove-partial-selectors))
        (remove-content-patterns (org-defuddle--option
                                  options
                                  :remove-content-patterns
                                  org-defuddle-remove-content-patterns))
        (remove-low-scoring (org-defuddle--option
                             options
                             :remove-low-scoring
                             org-defuddle-remove-low-scoring))
        (standardize (org-defuddle--standardize-option options))
        (debug (org-defuddle--debug-option options))
        (profile (org-defuddle--profile-option options))
        (frontmatter (org-defuddle--frontmatter-option options))
        (markdown (org-defuddle--markdown-option options))
        (separate-markdown (org-defuddle--separate-markdown-option options)))
    (json-parse-string
     (org-defuddle-module-parse-json-with-options html
                                                  (or url "")
                                                  (not (null include-images))
                                                  (not (null remove-small-images))
                                                  content-selector
                                                  include-replies
                                                  (not (null remove-hidden-elements))
                                                  (not (null remove-exact-selectors))
                                                  (not (null remove-partial-selectors))
                                                  (not (null remove-content-patterns))
                                                  (not (null remove-low-scoring))
                                                  (not (null standardize))
                                                  (not (null debug))
                                                  (not (null profile))
                                                  (not (null frontmatter))
                                                  (not (null markdown))
                                                  (not (null separate-markdown)))
     :object-type 'plist
     :array-type 'list
      :null-object nil
      :false-object nil)))

(defun org-defuddle-parse-html-property (html property &optional url options)
  "Parse HTML and return PROPERTY from the extracted response.

PROPERTY follows upstream defuddle's property names such as
\"title\", \"description\", \"domain\", \"wordCount\", and
\"parseTime\".  \"contentMarkdown\" returns first-pass Markdown content.
Snake-case aliases such as \"word_count\" are also accepted for
org-defuddle JSON fields.

URL is used for metadata and relative URL resolution.  OPTIONS accepts
the same keys as `org-defuddle-parse-html'."
  (org-defuddle-load-module)
  (unless (stringp property)
    (user-error "PROPERTY must be a string"))
  (let ((include-images (org-defuddle--include-images-option options))
        (remove-small-images (org-defuddle--option
                              options
                              :remove-small-images
                              org-defuddle-remove-small-images))
        (content-selector (org-defuddle--content-selector-option options))
        (include-replies (org-defuddle--include-replies-option options))
        (remove-hidden-elements (org-defuddle--option
                                 options
                                 :remove-hidden-elements
                                 org-defuddle-remove-hidden-elements))
        (remove-exact-selectors (org-defuddle--option
                                 options
                                 :remove-exact-selectors
                                 org-defuddle-remove-exact-selectors))
        (remove-partial-selectors (org-defuddle--option
                                   options
                                   :remove-partial-selectors
                                   org-defuddle-remove-partial-selectors))
        (remove-content-patterns (org-defuddle--option
                                  options
                                  :remove-content-patterns
                                  org-defuddle-remove-content-patterns))
        (remove-low-scoring (org-defuddle--option
                             options
                             :remove-low-scoring
                             org-defuddle-remove-low-scoring))
        (standardize (org-defuddle--standardize-option options))
        (debug (org-defuddle--debug-option options))
        (profile (org-defuddle--profile-option options))
        (frontmatter (org-defuddle--frontmatter-option options))
        (markdown (org-defuddle--markdown-option options))
        (separate-markdown
         (org-defuddle--separate-markdown-option options property)))
    (org-defuddle-module-parse-property-with-options
     html
     (or url "")
     property
     (not (null include-images))
     (not (null remove-small-images))
     content-selector
     include-replies
     (not (null remove-hidden-elements))
     (not (null remove-exact-selectors))
     (not (null remove-partial-selectors))
     (not (null remove-content-patterns))
     (not (null remove-low-scoring))
     (not (null standardize))
     (not (null debug))
     (not (null profile))
     (not (null frontmatter))
     (not (null markdown))
     (not (null separate-markdown)))))

(defun org-defuddle-parse-c2-json (json &optional url)
  "Parse C2 Wiki API JSON and return extracted metadata and Org.

The returned plist has keys matching the Rust `DefuddleOutput'
fields, including `:title', `:url', `:word_count', `:html', and
`:org'.  URL is the original C2 Wiki page URL."
  (org-defuddle-load-module)
  (json-parse-string
   (org-defuddle-module-parse-c2-json json (or url ""))
   :object-type 'plist
   :array-type 'list
   :null-object nil
   :false-object nil))

(defun org-defuddle-c2-json-to-org (json &optional url)
  "Parse C2 Wiki API JSON and return an Org string.

URL is the original C2 Wiki page URL."
  (org-defuddle-load-module)
  (org-defuddle-module-parse-c2-org json (or url "")))

(defun org-defuddle-parse-x-oembed-json (json &optional url)
  "Parse X/Twitter oEmbed JSON and return extracted metadata and Org.

The returned plist has keys matching the Rust `DefuddleOutput'
fields.  URL is the original X/Twitter status URL."
  (org-defuddle-load-module)
  (json-parse-string
   (org-defuddle-module-parse-x-oembed-json json (or url ""))
   :object-type 'plist
   :array-type 'list
   :null-object nil
   :false-object nil))

(defun org-defuddle-x-oembed-json-to-org (json &optional url)
  "Parse X/Twitter oEmbed JSON and return an Org string.

URL is the original X/Twitter status URL."
  (org-defuddle-load-module)
  (org-defuddle-module-parse-x-oembed-org json (or url "")))

(defun org-defuddle-parse-fxtwitter-json (json &optional url)
  "Parse FxTwitter API JSON and return extracted metadata and Org.

The returned plist has keys matching the Rust `DefuddleOutput'
fields.  URL is the original X/Twitter status or article URL."
  (org-defuddle-load-module)
  (json-parse-string
   (org-defuddle-module-parse-fxtwitter-json json (or url ""))
   :object-type 'plist
   :array-type 'list
   :null-object nil
   :false-object nil))

(defun org-defuddle-fxtwitter-json-to-org (json &optional url)
  "Parse FxTwitter API JSON and return an Org string.

URL is the original X/Twitter status or article URL."
  (org-defuddle-load-module)
  (org-defuddle-module-parse-fxtwitter-org json (or url "")))

(defun org-defuddle-bilibili-video-info (view-json &optional url)
  "Parse Bilibili VIEW-JSON and return request metadata.

URL is the original Bilibili page URL."
  (org-defuddle-load-module)
  (json-parse-string
   (org-defuddle-module-bilibili-video-info view-json (or url ""))
   :object-type 'plist
   :array-type 'list
   :null-object nil
   :false-object nil))

(defun org-defuddle-bilibili-subtitle-info (player-json &optional language)
  "Parse Bilibili PLAYER-JSON and return selected subtitle info.

LANGUAGE is an optional preferred language code.  The return value is
nil when no supported subtitle track is present."
  (org-defuddle-load-module)
  (json-parse-string
   (org-defuddle-module-bilibili-subtitle-info player-json (or language ""))
   :object-type 'plist
   :array-type 'list
   :null-object nil
   :false-object nil))

(defun org-defuddle-parse-bilibili-json (view-json &optional subtitle-json url language-code)
  "Parse Bilibili API JSON and return extracted metadata and Org.

VIEW-JSON is the response from the view API.  SUBTITLE-JSON is the
selected subtitle response, or nil for video metadata without transcript.
URL is the original Bilibili page URL.  LANGUAGE-CODE is the selected
subtitle language."
  (org-defuddle-load-module)
  (json-parse-string
   (org-defuddle-module-parse-bilibili-json view-json
                                            (or subtitle-json "")
                                            (or url "")
                                            (or language-code ""))
   :object-type 'plist
   :array-type 'list
   :null-object nil
   :false-object nil))

(defun org-defuddle-bilibili-json-to-org (view-json &optional subtitle-json url language-code)
  "Parse Bilibili API JSON and return an Org string.

VIEW-JSON is the response from the view API.  SUBTITLE-JSON is the
selected subtitle response, or nil for video metadata without transcript.
URL is the original Bilibili page URL.  LANGUAGE-CODE is the selected
subtitle language."
  (org-defuddle-load-module)
  (org-defuddle-module-parse-bilibili-org view-json
                                          (or subtitle-json "")
                                          (or url "")
                                          (or language-code "")))

(defun org-defuddle-youtube-caption-info (player-json &optional language)
  "Parse YouTube PLAYER-JSON and return selected caption info.

LANGUAGE is an optional preferred language code.  The return value is nil
when no supported caption track is present."
  (org-defuddle-load-module)
  (json-parse-string
   (org-defuddle-module-youtube-caption-info player-json (or language ""))
   :object-type 'plist
   :array-type 'list
   :null-object nil
   :false-object nil))

(defun org-defuddle-youtube-inline-caption-info (html url &optional language)
  "Return inline YouTube caption info parsed from HTML for URL.

LANGUAGE is an optional preferred language code.  The return value is nil
when the page has no current, supported caption track or the installed module
predates inline player-data support."
  (org-defuddle-load-module)
  (when (fboundp 'org-defuddle-module-youtube-inline-caption-info)
    (json-parse-string
     (org-defuddle-module-youtube-inline-caption-info html url (or language ""))
     :object-type 'plist
     :array-type 'list
     :null-object nil
     :false-object nil)))

(defun org-defuddle-parse-youtube-json
    (player-json &optional caption-xml chapters-json url language-code)
  "Parse YouTube API data and return extracted metadata and Org.

PLAYER-JSON is an Innertube player response.  CAPTION-XML is the selected
timedtext response, or nil when no transcript is available.  CHAPTERS-JSON
is an Innertube next response, URL is the original page URL, and
LANGUAGE-CODE identifies the selected caption track."
  (org-defuddle-load-module)
  (json-parse-string
   (org-defuddle-module-parse-youtube-json player-json
                                           (or caption-xml "")
                                           (or chapters-json "")
                                           (or url "")
                                           (or language-code ""))
   :object-type 'plist
   :array-type 'list
   :null-object nil
   :false-object nil))

(defun org-defuddle-youtube-json-to-org
    (player-json &optional caption-xml chapters-json url language-code)
  "Parse YouTube API data and return an Org string.

PLAYER-JSON is an Innertube player response.  CAPTION-XML is the selected
timedtext response, or nil when no transcript is available.  CHAPTERS-JSON
is an Innertube next response, URL is the original page URL, and
LANGUAGE-CODE identifies the selected caption track."
  (org-defuddle-load-module)
  (org-defuddle-module-parse-youtube-org player-json
                                         (or caption-xml "")
                                         (or chapters-json "")
                                         (or url "")
                                         (or language-code "")))

(defun org-defuddle-html-to-org (html &optional url options)
  "Parse HTML and return an Org string.

URL is used for metadata and relative URL resolution.  OPTIONS is a
plist.  `:include-images' overrides `org-defuddle-include-images',
`:remove-images' is accepted as its upstream-compatible inverse,
`:remove-small-images' overrides `org-defuddle-remove-small-images',
`:content-selector' overrides `org-defuddle-content-selector', and
`:include-replies' overrides `org-defuddle-include-replies'.
`:remove-hidden-elements' overrides
`org-defuddle-remove-hidden-elements'.  `:remove-exact-selectors'
and `:remove-partial-selectors' override their matching defcustoms.
`:remove-content-patterns' overrides
`org-defuddle-remove-content-patterns'.  `:remove-low-scoring'
overrides `org-defuddle-remove-low-scoring'.  `:standardize'
overrides `org-defuddle-standardize'.  `:debug' overrides
`org-defuddle-debug'.  `:profile' overrides `org-defuddle-profile'.
`:frontmatter' overrides `org-defuddle-frontmatter'.  `:markdown'
overrides `org-defuddle-markdown'.  `:separate-markdown' and
`:separateMarkdown' override `org-defuddle-separate-markdown'."
  (org-defuddle-load-module)
  (let ((include-images (org-defuddle--include-images-option options))
        (remove-small-images (org-defuddle--option
                              options
                              :remove-small-images
                              org-defuddle-remove-small-images))
        (content-selector (org-defuddle--content-selector-option options))
        (include-replies (org-defuddle--include-replies-option options))
        (remove-hidden-elements (org-defuddle--option
                                 options
                                 :remove-hidden-elements
                                 org-defuddle-remove-hidden-elements))
        (remove-exact-selectors (org-defuddle--option
                                 options
                                 :remove-exact-selectors
                                 org-defuddle-remove-exact-selectors))
        (remove-partial-selectors (org-defuddle--option
                                   options
                                   :remove-partial-selectors
                                   org-defuddle-remove-partial-selectors))
        (remove-content-patterns (org-defuddle--option
                                  options
                                  :remove-content-patterns
                                  org-defuddle-remove-content-patterns))
        (remove-low-scoring (org-defuddle--option
                             options
                             :remove-low-scoring
                             org-defuddle-remove-low-scoring))
        (standardize (org-defuddle--standardize-option options))
        (debug (org-defuddle--debug-option options))
        (profile (org-defuddle--profile-option options))
        (frontmatter (org-defuddle--frontmatter-option options))
        (markdown (org-defuddle--markdown-option options))
        (separate-markdown (org-defuddle--separate-markdown-option options)))
    (org-defuddle-module-parse-org-with-options html
                                                (or url "")
                                                (not (null include-images))
                                                (not (null remove-small-images))
                                                content-selector
                                                include-replies
                                                (not (null remove-hidden-elements))
                                                (not (null remove-exact-selectors))
                                                (not (null remove-partial-selectors))
                                                (not (null remove-content-patterns))
                                                (not (null remove-low-scoring))
                                                (not (null standardize))
                                                (not (null debug))
                                                (not (null profile))
                                                (not (null frontmatter))
                                                (not (null markdown))
                                                (not (null separate-markdown)))))

(defun org-defuddle-html-to-markdown (html &optional url options)
  "Parse HTML and return a Markdown string.

URL is used for metadata and relative URL resolution.  OPTIONS accepts
the same keys as `org-defuddle-parse-html'."
  (org-defuddle-parse-html-property
   html
   "contentMarkdown"
   url
   (append '(:separate-markdown t) options)))

;;;###autoload
(defun org-defuddle-buffer-to-org (&optional url options)
  "Extract the current buffer's HTML into a new Org buffer.

  URL is used for metadata and relative URL resolution.  OPTIONS is passed
  to `org-defuddle-html-to-org'."
  (interactive)
  (when (called-interactively-p 'interactive)
    (org-defuddle-load-module t))
  (let* ((html (buffer-substring-no-properties (point-min) (point-max)))
         (org (org-defuddle-html-to-org html url options)))
    (org-defuddle--insert-org-buffer org)))

(defun org-defuddle--html-url-to-org (url options)
  "Fetch URL as HTML and insert extracted Org using OPTIONS."
  (if (org-defuddle--cli-compatible-html-url-options-p options)
      (org-defuddle--cli-url-to-org url options)
    (org-defuddle--retrieve-body
     url
     (lambda (body)
       (org-defuddle--insert-org-buffer
        (org-defuddle-html-to-org body url options))))))

(defun org-defuddle--c2-url-to-org (url title)
  "Fetch C2 Wiki URL using TITLE and insert extracted Org."
  (org-defuddle--retrieve-body
   (org-defuddle--c2-api-url title)
   (lambda (body)
     (org-defuddle--insert-org-buffer
      (org-defuddle-c2-json-to-org body url)))))

(defun org-defuddle--x-url-to-org (url options match)
  "Fetch X/Twitter URL using async API fallback and insert Org.

MATCH is the parsed status match from `org-defuddle--x-status-match'.
OPTIONS is used only if both async fallbacks fail and the original HTML
URL must be parsed."
  (org-defuddle--retrieve-body
   (org-defuddle--fxtwitter-api-url match)
   (lambda (body)
     (condition-case nil
         (org-defuddle--insert-org-buffer
          (org-defuddle-fxtwitter-json-to-org body url))
       (error
        (org-defuddle--retrieve-body
         (org-defuddle--x-oembed-api-url url)
         (lambda (oembed-body)
           (condition-case nil
               (org-defuddle--insert-org-buffer
                (org-defuddle-x-oembed-json-to-org oembed-body url))
             (error
              (org-defuddle--html-url-to-org url options))))))))
   '(("User-Agent" . "Mozilla/5.0 (compatible; Defuddle/1.0; +https://defuddle.md)"))))

(defun org-defuddle--bilibili-insert-result (view-json subtitle-json url language)
  "Insert Bilibili Org parsed from VIEW-JSON and SUBTITLE-JSON.

URL is the original Bilibili page URL.  LANGUAGE is the selected subtitle
language code."
  (org-defuddle--insert-org-buffer
   (org-defuddle-bilibili-json-to-org view-json subtitle-json url language)))

(defun org-defuddle--bilibili-fetch-subtitle (view-json url language info)
  "Fetch Bilibili subtitle selected from INFO and insert Org.

VIEW-JSON is the Bilibili view API response.  URL is the original page
URL.  LANGUAGE is the preferred language code."
  (let ((api-urls (org-defuddle--bilibili-player-api-urls info)))
    (if api-urls
        (org-defuddle--bilibili-fetch-player-chain view-json url language api-urls)
      (org-defuddle--bilibili-insert-result view-json nil url ""))))

(defun org-defuddle--bilibili-fetch-player-chain (view-json url language api-urls)
  "Fetch Bilibili player API-URLS until a subtitle track is found.

VIEW-JSON is the Bilibili view API response.  URL is the original page
URL.  LANGUAGE is the preferred language code."
  (if (null api-urls)
      (org-defuddle--bilibili-insert-result view-json nil url "")
    (org-defuddle--retrieve-body
     (car api-urls)
     (lambda (player-json)
       (let ((subtitle-info (ignore-errors
                              (org-defuddle-bilibili-subtitle-info player-json language))))
         (if (and subtitle-info (plist-get subtitle-info :subtitle_url))
             (org-defuddle--retrieve-body
              (plist-get subtitle-info :subtitle_url)
              (lambda (subtitle-json)
                (condition-case nil
                    (org-defuddle--bilibili-insert-result
                     view-json
                     subtitle-json
                     url
                     (or (plist-get subtitle-info :language) ""))
                  (error
                   (org-defuddle--bilibili-insert-result view-json nil url "")))))
           (org-defuddle--bilibili-fetch-player-chain
            view-json
            url
            language
            (cdr api-urls)))))
     '(("Accept" . "application/json")
       ("User-Agent" . "Mozilla/5.0 (compatible; Defuddle/1.0)")))))

(defun org-defuddle--bilibili-url-to-org (url options bvid)
  "Fetch Bilibili URL using API extraction and insert Org.

OPTIONS supplies `:language' when present.  BVID is the parsed video id."
  (let ((language (org-defuddle--language-option options)))
    (org-defuddle--retrieve-body
     (org-defuddle--bilibili-view-api-url bvid)
     (lambda (view-json)
       (condition-case nil
           (let ((info (org-defuddle-bilibili-video-info view-json url)))
             (org-defuddle--bilibili-fetch-subtitle view-json url language info))
         (error
          (org-defuddle--html-url-to-org url options))))
     '(("Accept" . "application/json")
       ("User-Agent" . "Mozilla/5.0 (compatible; Defuddle/1.0)")))))

(defun org-defuddle--youtube-output-has-transcript-p (output)
  "Return non-nil when parsed YouTube OUTPUT contains a transcript."
  (let ((transcript (plist-get (plist-get output :variables) :transcript)))
    (and (stringp transcript) (not (string-empty-p transcript)))))

(defun org-defuddle--youtube-insert-result
    (player-json caption-xml chapters-json url selected-language
                 preferred-language options video-id remaining-configs)
  "Insert parsed YouTube Org, or continue trying another client.

PLAYER-JSON, CAPTION-XML, and CHAPTERS-JSON are fetched response bodies.
URL and VIDEO-ID identify the video.  SELECTED-LANGUAGE identifies the
caption track, while PREFERRED-LANGUAGE is the requested language.
OPTIONS is used for fallback, and REMAINING-CONFIGS are the untried
Innertube clients."
  (condition-case nil
      (let ((output (org-defuddle-parse-youtube-json
                     player-json caption-xml chapters-json url selected-language)))
        (if (org-defuddle--youtube-output-has-transcript-p output)
            (org-defuddle--insert-org-buffer (plist-get output :org))
          (org-defuddle--youtube-fetch-player-chain
           url video-id preferred-language options remaining-configs)))
    (error
     (org-defuddle--youtube-fetch-player-chain
      url video-id preferred-language options remaining-configs))))

(defun org-defuddle--youtube-fetch-caption
    (player-json chapters-json url video-id preferred-language options
                 caption-info remaining-configs)
  "Fetch the YouTube caption selected by CAPTION-INFO and insert Org.

PLAYER-JSON and CHAPTERS-JSON are prior API responses.  URL and VIDEO-ID
identify the video.  PREFERRED-LANGUAGE is the requested language,
OPTIONS is used for fallback, and REMAINING-CONFIGS are the untried
Innertube clients."
  (let ((caption-url (plist-get caption-info :caption_url))
        (selected-language (or (plist-get caption-info :language) "")))
    (if (and caption-url (not (string= caption-url "")))
        (org-defuddle--retrieve-body
         caption-url
         (lambda (caption-xml)
           (org-defuddle--youtube-insert-result
            player-json
            caption-xml
            chapters-json
            url
            selected-language
            preferred-language
            options
            video-id
            remaining-configs))
         (append
          '(("User-Agent" . "Mozilla/5.0"))
          (when (not (string= preferred-language ""))
            `(("Accept-Language" . ,preferred-language)))))
      (org-defuddle--youtube-fetch-player-chain
       url video-id preferred-language options remaining-configs))))

(defun org-defuddle--youtube-fetch-chapters
    (player-json url video-id language options caption-info remaining-configs)
  "Fetch YouTube chapters, then the caption selected by CAPTION-INFO.

PLAYER-JSON is the selected Innertube player response.  URL and VIDEO-ID
identify the video.  LANGUAGE is preferred and OPTIONS is used for
fallback.  REMAINING-CONFIGS are the untried Innertube clients."
  (let ((body (org-defuddle--youtube-request-body
               video-id "WEB" "2.20240101.00.00")))
    (org-defuddle--retrieve-body
     (org-defuddle--youtube-next-api-url)
     (lambda (chapters-json)
       (let ((validated-json
              (condition-case nil
                  (let ((_parsed (json-parse-string chapters-json)))
                    chapters-json)
                (error "{}"))))
         (org-defuddle--youtube-fetch-caption
          player-json
          validated-json
          url
          video-id
          language
          options
          caption-info
          remaining-configs)))
     (org-defuddle--youtube-api-headers language)
     "POST"
     body)))

(defun org-defuddle--youtube-insert-html-fallback (page-html url options)
  "Insert YouTube PAGE-HTML parsed for URL using OPTIONS."
  (org-defuddle--insert-org-buffer
   (org-defuddle-html-to-org page-html url options)))

(defun org-defuddle--youtube-insert-inline-result
    (page-html player-json caption-xml chapters-json url language options)
  "Insert an inline-caption YouTube result or its static page fallback.

PAGE-HTML is the fetched YouTube document.  PLAYER-JSON, CAPTION-XML,
and CHAPTERS-JSON are the inline player and fetched timedtext responses.
URL identifies the video, LANGUAGE identifies the caption track, and
OPTIONS configures static HTML extraction."
  (condition-case nil
      (let ((output (org-defuddle-parse-youtube-json
                     player-json caption-xml chapters-json url language)))
        (if (org-defuddle--youtube-output-has-transcript-p output)
            (org-defuddle--insert-org-buffer (plist-get output :org))
          (org-defuddle--youtube-insert-html-fallback page-html url options)))
    (error
     (org-defuddle--youtube-insert-html-fallback page-html url options))))

(defun org-defuddle--youtube-fetch-inline-caption
    (page-html player-json chapters-json url selected-language
               preferred-language options caption-url)
  "Fetch CAPTION-URL selected from PAGE-HTML and insert YouTube Org.

PLAYER-JSON and CHAPTERS-JSON are the matching inline player and fetched
chapter responses.  URL identifies the video.  SELECTED-LANGUAGE
identifies the caption track, PREFERRED-LANGUAGE is the requested
language, and OPTIONS configures fallback extraction."
  (org-defuddle--retrieve-body
   caption-url
   (lambda (caption-xml)
     (org-defuddle--youtube-insert-inline-result
      page-html
      player-json
      caption-xml
      chapters-json
      url
      selected-language
      options))
   (append
    '(("User-Agent" . "Mozilla/5.0"))
    (when (not (string= preferred-language ""))
      `(("Accept-Language" . ,preferred-language))))))

(defun org-defuddle--youtube-fetch-inline-chapters
    (page-html url video-id language options caption-info)
  "Fetch chapters before using inline YouTube CAPTION-INFO.

PAGE-HTML is the fetched YouTube document.  URL and VIDEO-ID identify the
video, LANGUAGE is preferred, and OPTIONS configures fallback extraction."
  (let ((body (org-defuddle--youtube-request-body
               video-id "WEB" "2.20240101.00.00"))
        (player-json (plist-get caption-info :player_json))
        (caption-url (plist-get caption-info :caption_url))
        (selected-language (or (plist-get caption-info :language) "")))
    (org-defuddle--retrieve-body
     (org-defuddle--youtube-next-api-url)
     (lambda (chapters-json)
       (let ((validated-json
              (condition-case nil
                  (let ((_parsed (json-parse-string chapters-json)))
                    chapters-json)
                (error "{}"))))
         (org-defuddle--youtube-fetch-inline-caption
          page-html
          player-json
          validated-json
          url
          selected-language
          language
          options
          caption-url)))
     (org-defuddle--youtube-api-headers language)
     "POST"
     body)))

(defun org-defuddle--youtube-fetch-inline-fallback
    (url video-id language options)
  "Try inline YouTube player data before falling back to static HTML.

URL and VIDEO-ID identify the video.  LANGUAGE is preferred and OPTIONS
configures extraction."
  (org-defuddle--retrieve-body
   url
   (lambda (page-html)
     (let ((caption-info
            (ignore-errors
              (org-defuddle-youtube-inline-caption-info
               page-html url language))))
       (if (and caption-info
                (plist-get caption-info :player_json)
                (plist-get caption-info :caption_url))
           (org-defuddle--youtube-fetch-inline-chapters
            page-html url video-id language options caption-info)
         (org-defuddle--youtube-insert-html-fallback page-html url options))))))

(defun org-defuddle--youtube-fetch-player-chain
    (url video-id language options configs)
  "Try YouTube Innertube client CONFIGS until a caption track is found.

URL and VIDEO-ID identify the video.  LANGUAGE is preferred and OPTIONS
is used for the HTML fallback."
  (if (null configs)
      (org-defuddle--youtube-fetch-inline-fallback
       url video-id language options)
    (let* ((config (car configs))
           (name (plist-get config :name))
           (version (plist-get config :version))
           (user-agent (plist-get config :user-agent))
           (body (org-defuddle--youtube-request-body video-id name version)))
      (org-defuddle--retrieve-body
       (org-defuddle--youtube-player-api-url)
       (lambda (player-json)
         (let ((caption-info
                (ignore-errors
                  (org-defuddle-youtube-caption-info player-json language))))
           (if (and caption-info (plist-get caption-info :caption_url))
               (org-defuddle--youtube-fetch-chapters
                player-json
                url
                video-id
                language
                options
                caption-info
                (cdr configs))
             (org-defuddle--youtube-fetch-player-chain
              url video-id language options (cdr configs)))))
       (org-defuddle--youtube-api-headers language user-agent)
       "POST"
       body))))

(defun org-defuddle--youtube-url-to-org (url options video-id)
  "Fetch YouTube URL using API transcript extraction and insert Org.

OPTIONS supplies `:language' when present.  VIDEO-ID is the parsed video
identifier."
  (org-defuddle--youtube-fetch-player-chain
   url
   video-id
   (org-defuddle--language-option options)
   options
   (org-defuddle--youtube-player-configs)))

(defun org-defuddle--reddit-url-to-org (url options old-url)
  "Fetch Reddit URL through OLD-URL and insert extracted Org.

URL is the original Reddit comments URL.  OPTIONS is passed to the Rust
HTML extraction path."
  (org-defuddle--retrieve-body
   old-url
   (lambda (body)
     (condition-case nil
         (org-defuddle--insert-org-buffer
          (org-defuddle-html-to-org body url options))
       (error
        (org-defuddle--html-url-to-org url options))))
   '(("User-Agent" . "Mozilla/5.0 (compatible; Defuddle/1.0)"))))

;;;###autoload
(defun org-defuddle-url-to-org (url &optional options)
  "Fetch URL and insert its extracted article into a new Org buffer.

OPTIONS is passed to `org-defuddle-html-to-org' for HTML pages.  C2 Wiki
and supported X/Twitter, Bilibili, YouTube, and Reddit URLs use API or
alternate-host extraction before falling back to their original HTML.
  `:use-async' overrides `org-defuddle-use-async'."
  (interactive "sURL: ")
  (when (called-interactively-p 'interactive)
    (org-defuddle-load-module t))
  (if (not (org-defuddle--use-async-option options))
      (org-defuddle--html-url-to-org url options)
    (let ((c2-title (org-defuddle--c2-page-title url))
          (x-match (org-defuddle--x-status-match url))
          (bilibili-bvid (org-defuddle--bilibili-bvid url))
          (youtube-video-id (org-defuddle--youtube-video-id url))
          (reddit-old-url (org-defuddle--reddit-old-comments-url url)))
      (cond
       (c2-title (org-defuddle--c2-url-to-org url c2-title))
       (x-match (org-defuddle--x-url-to-org url options x-match))
       (bilibili-bvid (org-defuddle--bilibili-url-to-org url options bilibili-bvid))
       (youtube-video-id
        (org-defuddle--youtube-url-to-org url options youtube-video-id))
       (reddit-old-url
        (org-defuddle--reddit-url-to-org url options reddit-old-url))
       (t (org-defuddle--html-url-to-org url options))))))

(provide 'org-defuddle)

;;; org-defuddle.el ends here
