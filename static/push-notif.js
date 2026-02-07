/**
 * SDK Push Notif gaya Pusher: channel + event + bind.
 * Pakai: PushNotif.subscribe('channel-name').bind('event-name', function(data) { ... })
 * Sebelum terima event, panggil PushNotif.requestSubscription() (atau klik Subscribe di halaman).
 */
(function (global) {
  'use strict';

  var API_BASE = (typeof global.PUSH_NOTIF_API_BASE !== 'undefined' ? global.PUSH_NOTIF_API_BASE : '');
  var channels = {};
  var channelList = [];
  var bindings = {};
  var vapidPublicKey = null;

  function getVapidPublicKey() {
    if (vapidPublicKey) return Promise.resolve(vapidPublicKey);
    return fetch(API_BASE + '/vapid-public-key')
      .then(function (r) { return r.json(); })
      .then(function (j) { vapidPublicKey = j.publicKey; return vapidPublicKey; });
  }

  function urlBase64ToUint8Array(base64String) {
    var padding = '='.repeat((4 - (base64String.length % 4)) % 4);
    var base64 = (base64String + padding).replace(/-/g, '+').replace(/_/g, '/');
    var rawData = window.atob(base64);
    var outputArray = new Uint8Array(rawData.length);
    for (var i = 0; i < rawData.length; ++i) outputArray[i] = rawData.charCodeAt(i);
    return outputArray;
  }

  function ensureChannel(name) {
    if (!channels[name]) {
      if (channelList.indexOf(name) === -1) channelList.push(name);
      channels[name] = { name: name, bindings: {} };
    }
    return channels[name];
  }

  function Channel(name) {
    this.name = name;
    this._bindings = {};
  }
  Channel.prototype.bind = function (eventName, callback) {
    var key = this.name + '::' + eventName;
    if (!bindings[key]) bindings[key] = [];
    bindings[key].push(callback);
    return this;
  };

  function dispatch(event, channel, data) {
    var key = channel + '::' + event;
    if (bindings[key]) {
      bindings[key].forEach(function (cb) {
        try { cb(data); } catch (e) { console.error(e); }
      });
    }
    key = '*::' + event;
    if (bindings[key]) {
      bindings[key].forEach(function (cb) {
        try { cb(data, channel); } catch (e) { console.error(e); }
      });
    }
  }

  function onPushReceived(payload) {
    if (payload.event && payload.channel != null) {
      dispatch(payload.event, payload.channel, payload.data || {});
    }
    if (payload.title != null || payload.body != null) {
      if (typeof global.onPushNotification === 'function') {
        global.onPushNotification({ title: payload.title, body: payload.body, data: payload.data });
      }
    }
  }

  function requestSubscription() {
    var chanList = channelList.length ? channelList : ['default'];
    return (Notification.requestPermission ? Notification.requestPermission() : Promise.resolve('denied'))
      .then(function (permission) {
        if (permission !== 'granted') return Promise.reject(new Error('Permission ' + permission));
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
        return fetch(API_BASE + '/subscribe', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ endpoint: raw.endpoint, keys: raw.keys, channels: chanList })
        });
      })
      .then(function (r) {
        if (!r.ok) return Promise.reject(new Error(r.statusText));
        return r.json();
      });
  }

  function trigger(channelsToSend, eventName, data) {
    return fetch(API_BASE + '/trigger', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        channels: Array.isArray(channelsToSend) ? channelsToSend : [channelsToSend],
        event: eventName,
        data: data || {}
      })
    }).then(function (r) { return r.json(); });
  }

  function subscribe(channelName) {
    ensureChannel(channelName);
    return new Channel(channelName);
  }

  if (typeof global.BroadcastChannel !== 'undefined') {
    var bc = new BroadcastChannel('web-push-alert');
    bc.onmessage = function (e) {
      if (e.data && e.data.type === 'push-received') onPushReceived(e.data);
    };
  }
  if (typeof global.addEventListener !== 'undefined') {
    global.addEventListener('message', function (e) {
      if (e.data && e.data.type === 'push-received') onPushReceived(e.data);
    });
  }

  global.PushNotif = {
    subscribe: subscribe,
    requestSubscription: requestSubscription,
    trigger: trigger,
    get channels() { return channelList.slice(); }
  };
})(typeof window !== 'undefined' ? window : this);
