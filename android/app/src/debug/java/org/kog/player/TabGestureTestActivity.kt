package org.kog.player

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.runtime.Composable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.ui.Modifier

/** Isolated surface for instrumentation of the production tab bar. */
class TabGestureTestActivity : ComponentActivity() {
    companion object { var content: (@Composable () -> Unit)? = null }
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent { Column(Modifier.fillMaxSize().statusBarsPadding()) { content?.invoke() } }
    }
}
