package com.fretec.studyhall.nativeplugin

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

class TimerActionReceiver : BroadcastReceiver() {
    companion object {
        const val ACTION_PAUSE = "com.fretec.studyhall.action.PAUSE"
        const val ACTION_RESUME = "com.fretec.studyhall.action.RESUME"
        const val ACTION_STOP = "com.fretec.studyhall.action.STOP"
    }

    override fun onReceive(context: Context, intent: Intent) {
        val action = intent.action ?: return
        val payloadAction = when (action) {
            ACTION_PAUSE -> "pause"
            ACTION_RESUME -> "resume"
            ACTION_STOP -> "stop"
            else -> return
        }
        StudyHallNativePlugin.instance?.dispatchAction(payloadAction)
        if (payloadAction == "stop") {
            TimerForegroundService.stop(context)
        }
    }
}
