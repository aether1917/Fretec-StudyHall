package com.fretec.studyhall.nativeplugin

import android.Manifest
import android.app.Activity
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class TimerArgs {
    var phase: String = "focus"
    var totalMs: Long = 0
    var startedAtMs: Long? = null
    var pauseOffsetMs: Long = 0
    var pausedAtMs: Long? = null
    var status: String = "idle"
}

@TauriPlugin
class StudyHallNativePlugin(private val activity: Activity) : Plugin(activity) {
    companion object {
        private const val NOTIFICATION_PERMISSION_REQUEST = 4201
        var instance: StudyHallNativePlugin? = null
            private set
    }

    init {
        instance = this
    }

    override fun load(webView: android.webkit.WebView) {
        super.load(webView)
    }

    @Command
    fun startTimer(invoke: Invoke) {
        val args = invoke.parseArgs(TimerArgs::class.java)
        TimerForegroundService.start(activity, args)
        invoke.resolve()
    }

    @Command
    fun updateTimer(invoke: Invoke) {
        val args = invoke.parseArgs(TimerArgs::class.java)
        TimerForegroundService.update(activity, args)
        invoke.resolve()
    }

    @Command
    fun stopTimer(invoke: Invoke) {
        TimerForegroundService.stop(activity)
        invoke.resolve()
    }

    @Command
    fun requestNotificationPermission(invoke: Invoke) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            val granted = ContextCompat.checkSelfPermission(
                activity,
                Manifest.permission.POST_NOTIFICATIONS,
            ) == PackageManager.PERMISSION_GRANTED
            if (!granted) {
                ActivityCompat.requestPermissions(
                    activity,
                    arrayOf(Manifest.permission.POST_NOTIFICATIONS),
                    NOTIFICATION_PERMISSION_REQUEST,
                )
            }
        }
        invoke.resolve()
    }

    @Command
    fun notificationPermissionStatus(invoke: Invoke) {
        val granted = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            ContextCompat.checkSelfPermission(
                activity,
                Manifest.permission.POST_NOTIFICATIONS,
            ) == PackageManager.PERMISSION_GRANTED
        } else {
            true
        }
        invoke.resolveObject(granted)
    }

    /** 通知按钮回调，转发给前端 / Rust 侧。 */
    fun dispatchAction(action: String) {
        try {
            val payload = JSObject()
            payload.put("action", action)
            trigger("timerAction", payload)
        } catch (_: Throwable) {
            // 前端不可用时忽略，通知自身仍会更新
        }
    }
}
