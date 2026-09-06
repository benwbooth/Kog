#include <lib/std.mi>

Function doFade();
Function stopFade();
Function startFade();

Global Group mainGroup;
Global Text tracktimer;
Global Boolean direction;
Global Timer myTimer;

System.onScriptLoaded() {
	mainGroup = getScriptGroup();
	tracktimer = mainGroup.findObject("SongTime");


	myTimer = new Timer;
	myTimer.setDelay(700);
	
	if(System.getStatus()==STATUS_PAUSED){
		startFade();
	}
}


System.onScriptUnloading(){
	delete myTimer;
}

myTimer.onTimer(){
	doFade();
}

System.onStop(){
	stopFade();
}
System.onPlay(){
	stopFade();
}
System.onResume(){
	stopFade();
}
System.onPause(){
	startFade();
}

doFade(){
	if(direction){
		tracktimer.setTargetA(0);
	}
	else{
		tracktimer.setTargetA(253);
	}
	tracktimer.setTargetSpeed(0.6);
	tracktimer.gotoTarget();
	direction=!direction;
}


startFade(){
	direction=true;
	doFade();
	myTimer.start();
}


stopFade(){
	myTimer.stop();
	tracktimer.cancelTarget();
	tracktimer.setAlpha(253);
}