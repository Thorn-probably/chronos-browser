/**
 * Chronos Browser Semantic Detector
 * In-page JS evaluated via CDP to detect active modals and interactive elements.
 * Returns a JSON-serializable object with page metadata, active modal context,
 * and a list of interactive elements with accurate bounding centers for clicks.
 */
(() => {
  // Helper to check element visibility
  function isVisible(el) {
    if (!el || !el.isConnected) return false;
    
    // Check aria-hidden
    if (el.closest('[aria-hidden="true"]')) return false;

    const style = window.getComputedStyle(el);
    if (style.display === 'none' || style.visibility === 'hidden' || style.visibility === 'collapse') {
      return false;
    }
    if (parseFloat(style.opacity) === 0) {
      return false;
    }

    const rect = el.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) {
      return false;
    }

    return true;
  }

  // Helper to compute accessible label/name
  function getElementLabel(el) {
    // 1. aria-label
    const ariaLabel = el.getAttribute('aria-label');
    if (ariaLabel && ariaLabel.trim()) return ariaLabel.trim();

    // 2. aria-labelledby
    const ariaLabelledby = el.getAttribute('aria-labelledby');
    if (ariaLabelledby) {
      const parts = ariaLabelledby.split(/\s+/).map(id => {
        const target = document.getElementById(id);
        return target ? (target.innerText || target.textContent || '').trim() : '';
      }).filter(Boolean);
      if (parts.length > 0) return parts.join(' ');
    }

    // 3. Associated <label> for inputs
    if (el.id) {
      const label = document.querySelector(`label[for="${CSS.escape(el.id)}"]`);
      if (label && label.innerText) return label.innerText.trim();
    }
    const parentLabel = el.closest('label');
    if (parentLabel) {
      // Clone label and remove element to get only label text
      const clone = parentLabel.cloneNode(true);
      const child = clone.querySelector(`[id="${CSS.escape(el.id)}"]`) || clone.querySelector(el.tagName.toLowerCase());
      if (child) child.remove();
      const txt = clone.innerText || clone.textContent;
      if (txt && txt.trim()) return txt.trim();
    }

    // 4. placeholder
    const placeholder = el.getAttribute('placeholder');
    if (placeholder && placeholder.trim()) return placeholder.trim();

    // 5. title attribute
    const title = el.getAttribute('title');
    if (title && title.trim()) return title.trim();

    // 6. alt attribute for img inside
    const img = el.querySelector('img[alt]');
    if (img && img.getAttribute('alt')) {
      const alt = img.getAttribute('alt').trim();
      if (alt) return alt;
    }

    // 7. value for submit/button inputs
    if (el.tagName === 'INPUT' && ['submit', 'button', 'reset'].includes(el.type.toLowerCase())) {
      if (el.value && el.value.trim()) return el.value.trim();
    }

    // 8. Text content / innerText
    let text = (el.innerText || el.textContent || '').trim();
    if (text) {
      // Normalize whitespace
      text = text.replace(/\s+/g, ' ');
      if (text.length > 100) {
        text = text.slice(0, 97) + '...';
      }
      return text;
    }

    // 9. name attribute as fallback
    const nameAttr = el.getAttribute('name');
    if (nameAttr && nameAttr.trim()) return nameAttr.trim();

    return '';
  }

  // 1. Detect active modals / overlays
  function detectActiveModal() {
    // Check standard HTML5 dialog
    const openDialogs = Array.from(document.querySelectorAll('dialog[open]')).filter(isVisible);
    if (openDialogs.length > 0) {
      const dialog = openDialogs[openDialogs.length - 1];
      return extractModalInfo(dialog, 'dialog');
    }

    // Check ARIA dialog or alertdialog
    const ariaDialogs = Array.from(document.querySelectorAll('[role="dialog"], [role="alertdialog"]')).filter(isVisible);
    if (ariaDialogs.length > 0) {
      const dialog = ariaDialogs[ariaDialogs.length - 1];
      const role = dialog.getAttribute('role') || 'dialog';
      return extractModalInfo(dialog, role);
    }

    // Check fixed/absolute elements with z-index >= 1000 and full backdrop or overlay
    const candidates = Array.from(document.querySelectorAll('body *')).filter(el => {
      if (!isVisible(el)) return false;
      const style = window.getComputedStyle(el);
      if (style.position !== 'fixed' && style.position !== 'absolute') return false;
      const zIndex = parseInt(style.zIndex, 10);
      if (isNaN(zIndex) || zIndex < 1000) return false;

      const rect = el.getBoundingClientRect();
      const coversViewport = (rect.width >= window.innerWidth * 0.6 && rect.height >= window.innerHeight * 0.6);
      const isCentredModal = (rect.width >= 200 && rect.height >= 100 &&
        rect.top >= 0 && rect.left >= 0 && rect.right <= window.innerWidth && rect.bottom <= window.innerHeight);

      return coversViewport || isCentredModal;
    });

    if (candidates.length > 0) {
      // Pick highest z-index or deepest candidate
      const modal = candidates[candidates.length - 1];
      return extractModalInfo(modal, 'overlay');
    }

    return null;
  }

  function extractModalInfo(el, role) {
    let title = '';
    const labelledby = el.getAttribute('aria-labelledby');
    if (labelledby) {
      const labelEl = document.getElementById(labelledby);
      if (labelEl) title = (labelEl.innerText || labelEl.textContent || '').trim();
    }
    if (!title) {
      title = el.getAttribute('aria-label') || '';
    }
    if (!title) {
      const heading = el.querySelector('h1, h2, h3, h4, h5, h6, [role="heading"], [class*="title" i], [class*="header" i]');
      if (heading) {
        title = (heading.innerText || heading.textContent || '').trim();
      }
    }
    if (!title) {
      title = (role === 'alertdialog' ? 'Alert Dialog' : 'Modal Dialog');
    }

    // Normalize title
    title = title.replace(/\s+/g, ' ');
    if (title.length > 80) {
      title = title.slice(0, 77) + '...';
    }

    const rect = el.getBoundingClientRect();
    return {
      element: el,
      info: {
        title: title,
        role: role,
        bounds: {
          x: Math.round(rect.x),
          y: Math.round(rect.y),
          width: Math.round(rect.width),
          height: Math.round(rect.height)
        }
      }
    };
  }

  const activeModalData = detectActiveModal();
  const activeModalEl = activeModalData ? activeModalData.element : null;
  const activeModal = activeModalData ? activeModalData.info : null;

  // 2. Query all interactive elements
  const interactiveSelectors = [
    'button',
    'a[href]',
    'input',
    'textarea',
    'select',
    '[role="button"]',
    '[role="link"]',
    '[role="checkbox"]',
    '[role="radio"]',
    '[role="switch"]',
    '[role="textbox"]',
    '[role="searchbox"]',
    '[role="combobox"]',
    '[role="listbox"]',
    '[role="menuitem"]',
    '[role="tab"]',
    '[contenteditable="true"]',
    '[contenteditable=""]',
    '[tabindex="0"]',
    '[data-clickable="true"]'
  ];

  const candidateNodes = Array.from(document.querySelectorAll(interactiveSelectors.join(', ')));

  // Include any extra contenteditable elements (e.g. WhatsApp Web)
  const contentEditables = Array.from(document.querySelectorAll('*')).filter(el => {
    return el.isContentEditable && !candidateNodes.includes(el);
  });
  candidateNodes.push(...contentEditables);

  const seenElements = new Set();
  const elements = [];

  for (const el of candidateNodes) {
    if (!isVisible(el)) continue;

    // Filter out hidden input elements
    if (el.tagName === 'INPUT' && el.type.toLowerCase() === 'hidden') continue;

    // Avoid nested duplicates (e.g., span inside button or inner icon)
    let parent = el.parentElement;
    let hasInteractiveParent = false;
    while (parent && parent !== document.body) {
      if (parent.tagName === 'BUTTON' || parent.tagName === 'A' || parent.getAttribute('role') === 'button') {
        hasInteractiveParent = true;
        break;
      }
      parent = parent.parentElement;
    }
    if (hasInteractiveParent && !['INPUT', 'SELECT', 'TEXTAREA'].includes(el.tagName)) {
      continue;
    }

    if (seenElements.has(el)) continue;
    seenElements.add(el);

    const tag = el.tagName.toLowerCase();
    const rect = el.getBoundingClientRect();
    const inViewport = (
      rect.bottom > 0 &&
      rect.top < window.innerHeight &&
      rect.right > 0 &&
      rect.left < window.innerWidth
    );

    const inModal = activeModalEl ? activeModalEl.contains(el) : false;

    // Determine role and action_type
    let role = el.getAttribute('role') || tag;
    let actionType = 'click';
    let checked = null;
    let value = null;
    let placeholder = el.getAttribute('placeholder') || null;

    if (el.isContentEditable || el.getAttribute('contenteditable') === 'true' || el.getAttribute('contenteditable') === '') {
      role = 'contenteditable';
      actionType = 'input';
      const textVal = (el.innerText || el.textContent || '').trim();
      value = textVal ? textVal.replace(/\s+/g, ' ') : '';
    } else if (tag === 'button' || role === 'button') {
      role = 'button';
      actionType = 'click';
    } else if (tag === 'a' || role === 'link') {
      role = 'link';
      actionType = 'link';
    } else if (tag === 'input') {
      const type = (el.type || 'text').toLowerCase();
      if (type === 'checkbox') {
        role = 'checkbox';
        actionType = 'toggle';
        checked = el.checked || el.getAttribute('aria-checked') === 'true';
      } else if (type === 'radio') {
        role = 'radio';
        actionType = 'toggle';
        checked = el.checked || el.getAttribute('aria-checked') === 'true';
      } else if (['submit', 'button', 'reset'].includes(type)) {
        role = 'button';
        actionType = 'click';
      } else {
        role = (type === 'search' ? 'searchbox' : 'input');
        actionType = 'input';
        value = el.value || '';
      }
    } else if (tag === 'textarea' || role === 'textbox' || role === 'searchbox') {
      role = (role === 'searchbox' ? 'searchbox' : 'textbox');
      actionType = 'input';
      value = el.value || (el.innerText || el.textContent || '').trim();
    } else if (tag === 'select' || role === 'combobox' || role === 'listbox') {
      role = 'combobox';
      actionType = 'select';
      if (tag === 'select' && el.selectedIndex >= 0 && el.options[el.selectedIndex]) {
        value = el.options[el.selectedIndex].text.trim();
      } else {
        value = el.value || '';
      }
    } else if (role === 'checkbox' || role === 'switch') {
      actionType = 'toggle';
      checked = el.getAttribute('aria-checked') === 'true';
    } else if (role === 'radio') {
      actionType = 'toggle';
      checked = el.getAttribute('aria-checked') === 'true';
    } else if (el.hasAttribute('onclick') || el.getAttribute('tabindex') === '0') {
      actionType = 'click';
      if (!el.getAttribute('role')) role = 'button';
    }

    const disabled = el.disabled === true || el.getAttribute('aria-disabled') === 'true';
    const name = getElementLabel(el);

    // Filter out completely unlabeled, empty div/span with no role
    if (!name && !value && !placeholder && !['input', 'checkbox', 'radio', 'combobox'].includes(role)) {
      continue;
    }

    const centerX = Math.round(rect.left + rect.width / 2);
    const centerY = Math.round(rect.top + rect.height / 2);

    elements.push({
      tag: tag,
      role: role,
      name: name,
      value: value,
      placeholder: placeholder,
      checked: checked,
      disabled: disabled,
      center: [centerX, centerY],
      bounds: {
        x: Math.round(rect.x),
        y: Math.round(rect.y),
        width: Math.round(rect.width),
        height: Math.round(rect.height)
      },
      in_modal: inModal,
      in_viewport: inViewport,
      action_type: actionType
    });
  }

  // Extract visible tables
  const tables = [];
  const tableElements = document.querySelectorAll('table');
  for (const tbl of tableElements) {
    if (!isVisible(tbl)) continue;
    const rows = [];
    const trElements = tbl.querySelectorAll('tr');
    for (const tr of trElements) {
      const cells = Array.from(tr.querySelectorAll('th, td'))
        .map(c => (c.innerText || c.textContent || '').trim().replace(/\s+/g, ' '))
        .filter(Boolean);
      if (cells.length > 0) {
        rows.push(cells.join(' | '));
      }
    }
    if (rows.length > 0) {
      tables.push(rows.slice(0, 20).join('\n'));
    }
  }

  return {
    title: document.title || '',
    url: window.location.href || '',
    modal: activeModal,
    elements: elements,
    tables: tables
  };
})();
