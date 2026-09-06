#include <lib/std.mi>

Function ani_play();
Function ani_pause();
Function ani_stop();

Global Group g, g_texttime;
Global Layer l_pbstatus;
Global Timer playfade, pausefade;
Global boolean pause_tog;

System.onScriptLoaded() {
	g = getScriptGroup();
	g_texttime = g.getObject("two.info.text.time");
	l_pbstatus = g.getObject("two.info.pbstatus");

	playfade = new Timer;
	playfade.setDelay(300);

	pausefade = new Timer;
	pausefade.setDelay(1000);
}

System.onscriptunloading(){
	delete playfade;
	delete pausefade;
}

// Stop animation
ani_stop(){
	l_pbstatus.cancelTarget();
	g_texttime.cancelTarget();
	pausefade.stop();
	l_pbstatus.setAlpha(0);
	l_pbstatus.show();
	g_texttime.setAlpha(255);
	l_pbstatus.setXmlParam("image", "info.bg.time.stop");
	l_pbstatus.setTargetA(255);
	g_texttime.setTargetA(0);
	l_pbstatus.setTargetSpeed(0.3);
	g_texttime.setTargetSpeed(0.4);
	l_pbstatus.gotoTarget();
	g_texttime.gotoTarget();
}
System.onStop(){
	ani_stop();
}

// Play/Resume animation
ani_play(){
	l_pbstatus.cancelTarget();
	g_texttime.cancelTarget();
	pausefade.stop();
	l_pbstatus.setAlpha(255);
	l_pbstatus.show();
	g_texttime.setAlpha(0);
	l_pbstatus.setXmlParam("image", "info.bg.time.play");
	playfade.start();
}
playfade.onTimer(){
	playfade.stop();
	l_pbstatus.setTargetA(0);
	g_texttime.setTargetA(255);
	l_pbstatus.setTargetSpeed(0.3);
	g_texttime.setTargetSpeed(0.4);
	l_pbstatus.gotoTarget();
	g_texttime.gotoTarget();
}
System.onPlay(){
	ani_play();
}
System.onResume(){
	ani_play();
}


// Pause animation
ani_pause(){
	l_pbstatus.cancelTarget();
	g_texttime.cancelTarget();
	pausefade.stop();
	l_pbstatus.setAlpha(0);
	l_pbstatus.show();
	g_texttime.setAlpha(255);
	l_pbstatus.setXmlParam("image", "info.bg.time.pause");
	pause_tog = true;
	pausefade.onTimer();
	pausefade.start();
}
pausefade.onTimer(){
	if(pause_tog){
		l_pbstatus.setTargetA(255);
		g_texttime.setTargetA(0);
	}
	else{
		l_pbstatus.setTargetA(0);
		g_texttime.setTargetA(255);
	}
	l_pbstatus.setTargetSpeed(0.5);
	g_texttime.setTargetSpeed(0.6);
	l_pbstatus.gotoTarget();
	g_texttime.gotoTarget();
	pause_tog = !pause_tog;
}
System.onPause(){
	ani_pause();
}