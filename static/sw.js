self.addEventListener('push', function (event) {
  var title = 'Notifikasi';
  var body = 'Pesan baru dari Web Push.';
  var icon = null;
  var payloadData = null;
  if (event.data) {
    try {
      var raw = typeof event.data.text === 'function' ? event.data.text() : (event.data.text || '');
      if (raw && raw.length > 0) {
        var data = JSON.parse(raw);
        payloadData = data;
        if (data.event && data.data) {
          if (data.data.title) title = String(data.data.title);
          if (data.data.body) body = String(data.data.body);
          if (data.data.icon) icon = String(data.data.icon);
          if (!title || title.length === 0) title = String(data.event);
          if (!body || body.length === 0) body = JSON.stringify(data.data);
        } else {
          if (data.title) title = String(data.title);
          if (data.body) body = String(data.body);
          if (data.icon) icon = String(data.icon);
        }
      }
    } catch (e) {
      try {
        if (event.data.json) {
          var data = event.data.json();
          payloadData = data;
          if (data && data.title) title = String(data.title);
          if (data && data.body) body = String(data.body);
          if (data && data.icon) icon = String(data.icon);
        }
      } catch (e2) {
        if (event.data.text) body = event.data.text() || body;
      }
    }
  }
  if (!title || title.length === 0) title = 'Notifikasi';
  if (!body || body.length === 0) body = 'Pesan baru.';

  // Default icon (same-origin) agar tidak pakai ikon browser (Firefox); data URL tidak didukung di Firefox
  if (!icon || icon.length === 0) {
    icon = new URL('static/icon-default.png', self.registration.scope).href;
  }

  var payload = { type: 'push-received', title: title, body: body };
  if (payloadData) {
    payload.event = payloadData.event;
    payload.channel = payloadData.channel;
    payload.data = payloadData.data;
    if (icon) payload.icon = icon;
  }
  var tag = 'web-push-' + Date.now();

  try {
    var channel = new BroadcastChannel('web-push-alert');
    channel.postMessage(payload);
  } catch (e) {}

  var notifOpts = { body: body, tag: tag, requireInteraction: false, icon: icon };

  function show(opts) {
    return self.registration.showNotification(title, opts || notifOpts);
  }

  // Coba dengan icon; jika gagal (icon load error dll), coba tanpa icon agar notifikasi tetap muncul
  var showPromise = show()
    .catch(function () {
      return show({ body: body, tag: tag, requireInteraction: false });
    });

  var notifyPromise = showPromise
    .then(function () {
      return self.clients.matchAll({ type: 'window', includeUncontrolled: true });
    })
    .then(function (clientList) {
      clientList.forEach(function (client) {
        if (client.postMessage) client.postMessage(payload);
      });
    })
    .catch(function () {});

  event.waitUntil(showPromise.then(function () { return notifyPromise; }).catch(function () {}));
});

self.addEventListener('notificationclick', function (event) {
  event.notification.close();
  event.waitUntil(
    clients.matchAll({ type: 'window', includeUncontrolled: true }).then(function (clientList) {
      if (clientList.length > 0 && clientList[0].focus) {
        return clientList[0].focus();
      }
      if (clients.openWindow) {
        return clients.openWindow('/');
      }
    })
  );
});
