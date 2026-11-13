package com.fretec.studyhall.nativeplugin

import android.Manifest
import android.app.Activity
import android.content.pm.PackageManager
import android.os.Build
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedCallback
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class FocusArgs {
    var active: Boolean = false
}

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
        val component = activity as? ComponentActivity ?: return
        component.onBackPressedDispatcher.addCallback(
            component,
            object : OnBackPressedCallback(true) {
                override fun handleOnBackPressed() {
                    trigger("backPressed", JSObject())
                }
            },
        )
    }

    @Command
    fun exitApp(invoke: Invoke) {
        activity.moveTaskToBack(true)
        invoke.resolve()
    }

    @Command
    fun focusMode(invoke: Invoke) {
        val args = invoke.parseArgs(FocusArgs::class.java)
        activity.runOnUiThread {
            val window = activity.window
            val controller = WindowCompat.getInsetsController(window, window.decorView)
            if (args.active) {
                window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
                controller.systemBarsBehavior =
                    WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
                controller.hide(WindowInsetsCompat.Type.systemBars())
                try {
                    activity.startLockTask()
                } catch (_: Exception) {
                    /* 用户拒绝屏幕固定时只保留全屏 */
                }
            } else {
                controller.show(WindowInsetsCompat.Type.systemBars())
                window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
                try {
                    activity.stopLockTask()
                } catch (_: Exception) {
                    /* 未处于固定模式 */
                }
            }
        }
        invoke.resolve()
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
