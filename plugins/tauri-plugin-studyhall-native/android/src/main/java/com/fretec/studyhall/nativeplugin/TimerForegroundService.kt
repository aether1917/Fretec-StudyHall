package com.fretec.studyhall.nativeplugin

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat

class TimerForegroundService : Service() {
    companion object {
        private const val CHANNEL_ID = "studyhall_timer"
        private const val NOTIFICATION_ID = 4202
        const val EXTRA_PHASE = "phase"
        const val EXTRA_TOTAL_MS = "totalMs"
        const val EXTRA_STARTED_AT_MS = "startedAtMs"
        const val EXTRA_PAUSE_OFFSET_MS = "pauseOffsetMs"
        const val EXTRA_PAUSED_AT_MS = "pausedAtMs"
        const val EXTRA_STATUS = "status"

        fun start(context: Context, args: TimerArgs) {
            val intent = Intent(context, TimerForegroundService::class.java).apply {
                putExtra(EXTRA_PHASE, args.phase)
                putExtra(EXTRA_TOTAL_MS, args.totalMs)
                putExtra(EXTRA_STARTED_AT_MS, args.startedAtMs ?: -1L)
                putExtra(EXTRA_PAUSE_OFFSET_MS, args.pauseOffsetMs)
                putExtra(EXTRA_PAUSED_AT_MS, args.pausedAtMs ?: -1L)
                putExtra(EXTRA_STATUS, args.status)
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
        }

        fun update(context: Context, args: TimerArgs) = start(context, args)

        fun stop(context: Context) {
            context.stopService(Intent(context, TimerForegroundService::class.java))
        }
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent == null) {
            stopSelf()
            return START_NOT_STICKY
        }
        val phase = intent.getStringExtra(EXTRA_PHASE) ?: "focus"
        val totalMs = intent.getLongExtra(EXTRA_TOTAL_MS, 0L)
        val startedAt = intent.getLongExtra(EXTRA_STARTED_AT_MS, -1L)
        val pauseOffset = intent.getLongExtra(EXTRA_PAUSE_OFFSET_MS, 0L)
        val pausedAt = intent.getLongExtra(EXTRA_PAUSED_AT_MS, -1L)
        val status = intent.getStringExtra(EXTRA_STATUS) ?: "idle"

        createChannel()
        val notification = buildNotification(phase, totalMs, startedAt, pauseOffset, pausedAt, status)
        startForeground(NOTIFICATION_ID, notification)
        return START_NOT_STICKY
    }

    private fun createChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        if (manager.getNotificationChannel(CHANNEL_ID) == null) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "学习计时",
                NotificationManager.IMPORTANCE_LOW,
            ).apply {
                setShowBadge(false)
                enableVibration(false)
            }
            manager.createNotificationChannel(channel)
        }
    }

    private fun buildNotification(
        phase: String,
        totalMs: Long,
        startedAt: Long,
        pauseOffset: Long,
        pausedAt: Long,
        status: String,
    ): Notification {
        val running = status == "running"
        val paused = status == "paused"
        val anchor = if (paused && pausedAt > 0) pausedAt else System.currentTimeMillis()
        val elapsed = if (startedAt > 0) anchor - startedAt - pauseOffset else 0L
        val remaining = (totalMs - elapsed).coerceAtLeast(0L)
        val endsAt = System.currentTimeMillis() + remaining

        val phaseLabel = if (phase == "break") "休息" else "专注"
        val remainingSec = (remaining / 1000L).toInt().coerceAtLeast(0)
        val clock = "%02d:%02d".format(remainingSec / 60, remainingSec % 60)
        val title = if (paused) "$phaseLabel · 已暂停 $clock" else "$phaseLabel · 剩余 $clock"
        val content = if (paused) "暂停中，剩余 $clock" else "${phaseLabel}中，剩余 $clock"

        val launchIntent = packageManager.getLaunchIntentForPackage(packageName)
        val contentIntent = launchIntent?.let {
            PendingIntent.getActivity(
                this,
                0,
                it,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )
        }

        val toggleAction = if (paused) TimerActionReceiver.ACTION_RESUME else TimerActionReceiver.ACTION_PAUSE
        val toggleIntent = PendingIntent.getBroadcast(
            this,
            1,
            Intent(this, TimerActionReceiver::class.java).setAction(toggleAction),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        val stopIntent = PendingIntent.getBroadcast(
            this,
            2,
            Intent(this, TimerActionReceiver::class.java).setAction(TimerActionReceiver.ACTION_STOP),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )

        val builder = NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle(title)
            .setContentText(content)
            .setSmallIcon(android.R.drawable.ic_lock_idle_alarm)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setSilent(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
            .addAction(0, if (paused) "继续" else "暂停", toggleIntent)
            .addAction(0, "结束", stopIntent)

        if (contentIntent != null) builder.setContentIntent(contentIntent)

        if (running) {
            builder
                .setUsesChronometer(true)
                .setChronometerCountDown(true)
                .setWhen(endsAt)
                .setShowWhen(true)
        }
        return builder.build()
    }
}
