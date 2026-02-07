$(function () {
  var API_BASE = '';
  var $status = $('#status');
  var totalTerkirim = 0;

  function showStatus(msg, isError) {
    $status.removeClass('error success').addClass(isError ? 'error' : 'success').text(msg).show();
  }

  function showNotifyAlert(title, body) {
    var msg = (title || '') + (body ? ': ' + body : '');
    if (msg) showStatus(msg, false);
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

  if (typeof PushNotif !== 'undefined') {
    PushNotif.subscribe('default').bind('test', function (data) {
      showStatus('Event "test" diterima: ' + JSON.stringify(data), false);
      if (data.title || data.body) showNotifyAlert(data.title || 'Event', data.body || '');
    });
    PushNotif.subscribe('notifications').bind('new-message', function (data) {
      showStatus('Event "new-message" (channel notifications): ' + JSON.stringify(data), false);
      if (data.title || data.body) showNotifyAlert(data.title || 'Pesan baru', data.body || '');
    });
  }

  $('#btn-subscribe').on('click', function () {
    var doSubscribe = typeof PushNotif !== 'undefined'
      ? PushNotif.requestSubscription.bind(PushNotif)
      : function () {
        var promise = Notification.requestPermission ? Notification.requestPermission() : Promise.resolve('denied');
        return promise.then(function (permission) {
          if (permission !== 'granted') return Promise.reject(new Error('Permission ' + permission));
          return getVapidPublicKey();
        }).then(function (publicKey) {
          return navigator.serviceWorker.ready.then(function (reg) {
            return reg.pushManager.subscribe({
              userVisibleOnly: true,
              applicationServerKey: urlBase64ToUint8Array(publicKey)
            });
          });
        }).then(function (subscription) {
          var raw = subscription.toJSON ? subscription.toJSON() : {
            endpoint: subscription.endpoint,
            keys: {
              p256dh: btoa(String.fromCharCode.apply(null, new Uint8Array(subscription.getKey('p256dh'))))
                .replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, ''),
              auth: btoa(String.fromCharCode.apply(null, new Uint8Array(subscription.getKey('auth'))))
                .replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '')
            }
          };
          return $.ajax({
            url: API_BASE + '/subscribe',
            method: 'POST',
            contentType: 'application/json',
            data: JSON.stringify({ endpoint: raw.endpoint, keys: raw.keys })
          });
        });
      };
    doSubscribe()
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
        showStatus('Notifikasi dikirim: ' + sent + ' berhasil, ' + failed + ' gagal. Total terkirim: ' + totalTerkirim + 'x', false);
      })
      .fail(function (xhr, status, err) {
        showStatus('Gagal mengirim: ' + (xhr.responseText || err), true);
      });
  });

  $('#btn-trigger').on('click', function () {
    if (typeof PushNotif === 'undefined') {
      showStatus('SDK PushNotif tidak dimuat.', true);
      return;
    }
    PushNotif.trigger(['default'], 'test', { title: 'Event test', body: 'Ini dari trigger (gaya Pusher).' })
      .then(function (r) {
        if (r.ok) showStatus('Trigger dikirim: ' + (r.sent || 0) + ' berhasil.', false);
        else showStatus('Trigger gagal: ' + (r.message || ''), true);
      })
      .catch(function (e) {
        showStatus('Trigger gagal: ' + (e.message || e), true);
      });
  });

  function getCurlCommand() {
    var origin = window.location.origin || 'http://127.0.0.1:3000';
    var url = origin + '/notify';
    return "curl -X POST " + url + " \\\n  -H \"Content-Type: application/json\" \\\n  -d '{\"title\":\"Test Notification\",\"body\":\"Ini notifikasi dari backend Rust.\"}'";
  }

  function getCurlTrigger() {
    var origin = window.location.origin || 'http://127.0.0.1:3000';
    return "curl -X POST " + origin + "/trigger \\\n  -H \"Content-Type: application/json\" \\\n  -d '{\"channels\":[\"default\"],\"event\":\"test\",\"data\":{\"title\":\"Hi\",\"body\":\"Pesan dari curl\"}}'";
  }

  function showCurlSection() {
    var cmd = getCurlCommand();
    $('#curl-cmd').text(cmd);
    var triggerCmd = getCurlTrigger ? getCurlTrigger() : '';
    if (triggerCmd && $('#curl-trigger').length) $('#curl-trigger').text(triggerCmd);
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
