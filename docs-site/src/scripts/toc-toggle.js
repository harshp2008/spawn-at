// Hides the "On this page" column on wide screens and widens the text. The file is read by
// astro.config.mjs and added to the <head> of every page as an inline script, so the saved choice
// is applied before the first paint (no flash of the wrong layout). The layout itself is the CSS
// in src/styles/toc-toggle.css, keyed on the attribute `data-toc-hidden` of <html>.
// Without JavaScript nothing here runs: the attribute is never set and the button is never made.
(function () {
  var KEY = 'spawn-at:toc-hidden';
  var root = document.documentElement;

  // localStorage throws in some private windows and when site data is blocked.
  function read() {
    try {
      return window.localStorage.getItem(KEY) === '1';
    } catch (e) {
      return false;
    }
  }
  function write(hidden) {
    try {
      window.localStorage.setItem(KEY, hidden ? '1' : '0');
    } catch (e) {
      // the choice then lasts until the page is closed
    }
  }
  function apply(hidden) {
    if (hidden) root.setAttribute('data-toc-hidden', '');
    else root.removeAttribute('data-toc-hidden');
  }

  apply(read());

  document.addEventListener('DOMContentLoaded', function () {
    var column = document.querySelector('.right-sidebar-container');
    var host = document.querySelector('.header .right-group');
    // A page without a table of contents has no column to hide.
    if (!column || !host) return;
    column.id = 'toc-column';

    var button = document.createElement('button');
    button.type = 'button';
    button.className = 'toc-toggle';
    button.setAttribute('aria-label', 'Hide the table of contents');
    button.setAttribute('aria-controls', 'toc-column');
    button.title = 'Hide the table of contents';
    button.innerHTML =
      '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" ' +
      'stroke-width="2" stroke-linecap="round" aria-hidden="true" focusable="false">' +
      '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M15 4v16"/></svg>';

    // aria-pressed is true while the column is hidden; the label does not change with it.
    function sync() {
      button.setAttribute('aria-pressed', root.hasAttribute('data-toc-hidden') ? 'true' : 'false');
    }
    sync();
    button.addEventListener('click', function () {
      var hidden = !root.hasAttribute('data-toc-hidden');
      apply(hidden);
      write(hidden);
      sync();
    });
    host.insertBefore(button, host.firstChild);
  });
})();
