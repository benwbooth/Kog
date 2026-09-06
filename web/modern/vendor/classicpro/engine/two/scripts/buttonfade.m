#include <lib/std.mi>

Function String getMyPath();

Global Group myGroup;
Global Button myButton;
Global ToggleButton myTogButton;
Global Layer myLayer;
Global Boolean isOver, isBut; //, isInv;
Global String myString;
Global Timer recheck;

System.onScriptLoaded() {
	myGroup = getScriptGroup();
	myButton = myGroup.getObject(getToken(getParam(),";",0));
	myTogButton = myGroup.getObject(getToken(getParam(),";",0));
	myLayer = myGroup.getObject(getToken(getParam(),";",1));
	
	
	if(myTogButton==null) isBut = true;
	
	if(!isBut){
		myString = getToken(getParam(),";",2);

		//if(myTogButton.getXmlParam("nstates")=="3") isInv = true;
		myTogButton.onToggle(true);
	}
}

myButton.onLeftButtonDown(int x, int y){
	myLayer.cancelTarget();
	myLayer.setAlpha(0);
}
myButton.onLeftButtonUp(int x, int y){
	myLayer.cancelTarget();
	if(myButton.isMouseOverRect() && myButton.isVisible()) myLayer.setAlpha(255);
	//debugint(myButton.isMouseOverRect());

	recheck = new Timer;
	recheck.setDelay(50);
	recheck.start();
}


myButton.onEnterArea(){
	if(!myButton.isVisible()) return;
	//isOver = true;
	
	myLayer.cancelTarget();
	myLayer.setAlpha(255);
	
	if(!isBut) myTogButton.onToggle(true);
	
	/*
	myLayer.setTargetA(255);
	myLayer.setTargetSpeed(0.2);
	myLayer.gotoTarget();
	*/
	
}
myButton.onLeaveArea(){
	if(!myButton.isVisible()) return;
	myLayer.cancelTarget();
	myLayer.setAlpha(255);
	myLayer.setTargetA(0);
	myLayer.setTargetSpeed(0.6);
	myLayer.gotoTarget();
}

myButton.onSetVisible(boolean onOff){
	//if(onOff && myButton.isMouseOver(0, 0)){
	if(onOff && myButton.isMouseOverRect()){
		//myLayer.cancelTarget();
		myLayer.setAlpha(255);
		recheck = new Timer;
		recheck.setDelay(50);
		recheck.start();
	}
	else{
		if(!myLayer.isGoingToTarget()){
			myLayer.cancelTarget();
			myLayer.setAlpha(0);
		}
		delete recheck;
	}
	
}
recheck.onTimer(){
	recheck.stop();

	if(myButton.isMouseOverRect() && !System.isMinimized()){
		//debug("123");
		//myLayer.cancelTarget();
		myLayer.setAlpha(255);
		//recheck = new Timer;
		//recheck.start();
	}
	else{
		myLayer.cancelTarget();
		myLayer.setAlpha(0);
	}

}


myTogButton.onToggle(Boolean onoff){
	if(isBut) return;

	int i = myTogButton.getCurCfgVal();
	if(i<0) i=1-i;
	
	//debugint(i);
	
	myLayer.setXmlParam("image", myString+integerToString(i));
}