$(function () {
  var API_BASE = '';
  var $status = $('#status');
  var totalTerkirim = 0;

  function showStatus(msg, isError) {
    $status.removeClass('error success').addClass(isError ? 'error' : 'success').text(msg).show();
  }

  function getVapidPublicKey() {
    return $.getJSON(API_BASE + '/vapid-public-key').then(function (r) {
      return r.publicKey;
    });
  }

  function urlBase64ToUint8Array(base64String) {
    var padding = '='.repeat((4 - (base64String.length % 4)) % 4);
    var base64 = (base64String + padding).replace(/-/g, '+').replace(/_/g, '/');
    var rawData = window.atob(base64);
    var outputArray = new Uint8Array(rawData.length);
    for (var i = 0; i < rawData.length; ++i) {
      outputArray[i] = rawData.charCodeAt(i);
    }
    return outputArray;
  }

  $('#btn-subscribe').on('click', function () {
    var promise = Notification.requestPermission ? Notification.requestPermission() : Promise.resolve('denied');
    promise.then(function (permission) {
      if (permission !== 'granted') {
        showStatus('Notifikasi ditolak. Izinkan notifikasi untuk browser ini lalu coba lagi.', true);
        return Promise.reject(new Error('Permission ' + permission));
      }
      return getVapidPublicKey();
    })
      .then(function (publicKey) {
        return navigator.serviceWorker.ready.then(function (reg) {
          return reg.pushManager.subscribe({
            userVisibleOnly: true,
            applicationServerKey: urlBase64ToUint8Array(publicKey)
          });
        });
      })
      .then(function (subscription) {
        var raw = subscription.toJSON ? subscription.toJSON() : {
          endpoint: subscription.endpoint,
          keys: {
            p256dh: btoa(String.fromCharCode.apply(null, new Uint8Array(subscription.getKey('p256dh'))))
              .replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, ''),
            auth: btoa(String.fromCharCode.apply(null, new Uint8Array(subscription.getKey('auth'))))
              .replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '')
          }
        };
        var body = { endpoint: raw.endpoint, keys: raw.keys };
        return $.ajax({
          url: API_BASE + '/subscribe',
          method: 'POST',
          contentType: 'application/json',
          data: JSON.stringify(body)
        });
      })
      .then(function () {
        showStatus('Subscribe berhasil. Tutup tab lalu jalankan curl — notifikasi akan muncul di sistem (pojok layar). Jangan quit browser.', false);
        showCurlSection();
      })
      .catch(function (xhrOrErr) {
        var msg = 'Subscribe gagal. ';
        if (xhrOrErr && xhrOrErr.status !== undefined) {
          if (xhrOrErr.status === 0) {
            msg = 'Backend tidak berjalan atau CORS error. Pastikan server Rust berjalan di http://127.0.0.1:3000';
          } else {
            msg += (xhrOrErr.responseJSON && xhrOrErr.responseJSON.message) || xhrOrErr.responseText || xhrOrErr.statusText || xhrOrErr;
          }
        } else {
          msg += xhrOrErr && (xhrOrErr.message || xhrOrErr);
        }
        showStatus(msg, true);
      });
  });

  $('#btn-test-local').on('click', function () {
    if (!('Notification' in window)) {
      showStatus('Browser tidak mendukung Notification API.', true);
      return;
    }
    if (Notification.permission !== 'granted') {
      showStatus('Izinkan notifikasi dulu (klik Subscribe).', true);
      return;
    }
    try {
      var n = new Notification('Test lokal', { body: 'Jika ini muncul, izin notifikasi OK. Push harusnya juga bisa.' });
      n.onclick = function () { n.close(); };
      showStatus('Notifikasi lokal ditampilkan. Cek pojok layar / ikon browser.', false);
    } catch (e) {
      showStatus('Gagal tampil notifikasi lokal: ' + (e.message || e), true);
    }
  });

  window.addEventListener('message', function (event) {
    if (event.data && event.data.type === 'push-received') {
      showStatus('Push diterima oleh browser.', false);
      showNotifyAlert(event.data.title, event.data.body);
    }
  });

  var pushChannel = new BroadcastChannel('web-push-alert');
  pushChannel.onmessage = function (event) {
    if (event.data && event.data.type === 'push-received') {
      showStatus('Push diterima (dari curl/backend).', false);
      showNotifyAlert(event.data.title, event.data.body);
    }
  };

  var lastSeenNotifyId = 0;
  setInterval(function () {
    if (document.visibilityState !== 'visible') return;
    $.getJSON(API_BASE + '/notify/last').then(function (r) {
      if (r.id != null && r.id > lastSeenNotifyId) {
        lastSeenNotifyId = r.id;
        showStatus('Notifikasi baru (dari curl).', false);
      }
    }).catch(function () {});
  }, 2000);

  var notifyTitle = 'Test Notification';
  var notifyBody = 'Ini notifikasi dari backend Rust.';

  $('#btn-notify').on('click', function () {
    $.ajax({
      url: API_BASE + '/notify',
      method: 'POST',
      contentType: 'application/json',
      data: JSON.stringify({ title: notifyTitle, body: notifyBody })
    })
      .then(function (r) {
        var sent = r.sent || 0;
        var failed = r.failed || 0;
        totalTerkirim += sent;
        if (r.id != null) lastSeenNotifyId = r.id;
        showStatus('Notifikasi dikirim: ' + sent + ' berhasil, ' + failed + ' gagal. Total terkirim: ' + totalTerkirim + 'x', false);
      })
      .fail(function (xhr, status, err) {
        showStatus('Gagal mengirim: ' + (xhr.responseText || err), true);
      });
  });

  function getCurlCommand() {
    var origin = window.location.origin || 'http://127.0.0.1:3000';
    var url = origin + '/notify';
    return "curl -X POST " + url + " \\\n  -H \"Content-Type: application/json\" \\\n  -d '{\"title\":\"Test Notification\",\"body\":\"Ini notifikasi dari backend Rust.\"}'";
  }

  function showCurlSection() {
    var cmd = getCurlCommand();
    $('#curl-cmd').text(cmd);
    $('#curl-section').show();
  }

  $('#btn-copy-curl').on('click', function () {
    var cmd = getCurlCommand();
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(cmd).then(function () {
        var $btn = $('#btn-copy-curl');
        var oldText = $btn.text();
        $btn.text('Tersalin!');
        setTimeout(function () { $btn.text(oldText); }, 1500);
      }).catch(function () {
        selectAndCopyFallback(cmd);
      });
    } else {
      selectAndCopyFallback(cmd);
    }
  });

  function selectAndCopyFallback(text) {
    var $pre = $('#curl-cmd');
    var range = document.createRange();
    range.selectNodeContents($pre[0]);
    var sel = window.getSelection();
    sel.removeAllRanges();
    sel.addRange(range);
    try {
      document.execCommand('copy');
      var $btn = $('#btn-copy-curl');
      var oldText = $btn.text();
      $btn.text('Tersalin!');
      setTimeout(function () { $btn.text(oldText); }, 1500);
    } catch (e) {}
    sel.removeAllRanges();
  }

  if ('serviceWorker' in navigator) {
    navigator.serviceWorker.register('/sw.js?v=4', { scope: '/' })
      .then(function (reg) {
        reg.update();
        if (reg.waiting) reg.waiting.postMessage({ type: 'SKIP_WAITING' });
      })
      .catch(function () {
        showStatus('Service Worker gagal didaftarkan. Buka lewat http://127.0.0.1:3000 (bukan file://).', true);
      });
  } else {
    showStatus('Browser tidak mendukung Service Worker / Push.', true);
  }
});
